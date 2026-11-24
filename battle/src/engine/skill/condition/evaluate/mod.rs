mod actions;
mod buff;
mod predicates;
mod repeat;
mod resources;
mod targets;

use predicates::condition_kind_matches;
use repeat::condition_repeat_count;
use targets::{
    entity_count_matches, other_ally_damage_type_count, per_target_career_count,
    per_team_career_count, team_career_count,
};

use crate::engine::{
    manager::BattleManagers,
    skill::{
        condition::{
            ConditionCompare, EntityCountScope, ParsedCondition, ParsedConditionKind,
            TargetIdentityMode, buff::BuffConditionMode,
        },
        rule::DefinitionKey,
        target::{TargetContext, TargetEntity, TargetPool},
    },
};

pub fn satisfied_conditions(
    conditions: &[ParsedCondition],
    trigger_key: DefinitionKey,
) -> Vec<ParsedCondition> {
    conditions
        .iter()
        .map(|condition| satisfied_condition(condition, trigger_key))
        .collect()
}

pub fn satisfied_card_enchants(
    conditions: &[ParsedCondition],
    card_enchants: &[i32],
) -> Vec<ParsedCondition> {
    conditions
        .iter()
        .map(|condition| {
            let mut condition = condition.clone();
            match &mut condition.kind {
                ParsedConditionKind::CurrentCardEnchant { enchant_id }
                    if !card_enchants.is_empty()
                        && (*enchant_id == 0 || card_enchants.contains(enchant_id)) =>
                {
                    condition = ParsedCondition::always();
                }
                ParsedConditionKind::Any(groups) => {
                    for group in groups {
                        *group = satisfied_card_enchants(group, card_enchants);
                    }
                }
                ParsedConditionKind::Not(inner) => {
                    let nested = ParsedCondition {
                        opcode: condition.opcode,
                        type_name: condition.type_name.clone(),
                        kind: *inner.clone(),
                        raw_args: condition.raw_args.clone(),
                    };
                    **inner = satisfied_card_enchants(std::slice::from_ref(&nested), card_enchants)
                        [0]
                    .kind
                    .clone();
                }
                _ => {}
            }
            condition
        })
        .collect()
}

pub fn satisfied_condition(
    condition: &ParsedCondition,
    trigger_key: DefinitionKey,
) -> ParsedCondition {
    let is_satisfiable_event_marker = match condition.kind {
        ParsedConditionKind::Lifecycle(_)
        | ParsedConditionKind::AllyAttacked
        | ParsedConditionKind::EntityDead => true,
        ParsedConditionKind::None(mode) => !matches!(
            mode,
            super::none::NoneMode::AllyAction | super::none::NoneMode::EmitterSkillAction
        ),
        _ => false,
    };
    if trigger_key.matches(condition.opcode, &condition.type_name)
        && (matches!(condition.timing(), super::ConditionTiming::Event(_))
            || matches!(
                super::registry::find_key(condition.opcode, &condition.type_name)
                    .map(|definition| definition.role),
                Some(super::registry::ConditionRole::Setup { .. })
            ))
        && is_satisfiable_event_marker
    {
        return ParsedCondition::always();
    }
    let mut condition = condition.clone();
    if let ParsedConditionKind::Any(groups) = &mut condition.kind {
        for group in groups {
            for condition in group {
                *condition = satisfied_condition(condition, trigger_key);
            }
        }
    }
    condition
}

pub fn conditions_match(
    conditions: &[ParsedCondition],
    source_uid: i64,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
    pool: &TargetPool,
    context: TargetContext,
) -> bool {
    conditions.iter().all(|condition| {
        condition_matches(
            condition,
            source_uid,
            condition_targets,
            managers,
            pool,
            context,
        )
    })
}

pub(crate) fn conditions_fire_count(
    conditions: &[ParsedCondition],
    source_uid: i64,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
    pool: &TargetPool,
    context: TargetContext,
) -> i32 {
    if conditions.is_empty() {
        return 1;
    }

    if !conditions_match(
        conditions,
        source_uid,
        condition_targets,
        managers,
        pool,
        context,
    ) {
        return 0;
    }

    conditions
        .iter()
        .filter_map(|condition| {
            condition_repeat_count(
                condition,
                source_uid,
                condition_targets,
                managers,
                pool,
                context,
            )
        })
        .min()
        .unwrap_or(1)
}

pub(crate) fn condition_matches(
    condition: &ParsedCondition,
    source_uid: i64,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
    pool: &TargetPool,
    context: TargetContext,
) -> bool {
    let matched = condition_kind_matches(
        condition,
        &condition.kind,
        source_uid,
        condition_targets,
        managers,
        pool,
        context,
    );
    if crate::engine::diagnostics::enabled(crate::engine::diagnostics::TraceArea::Condition) {
        eprintln!(
            "condition {} {} args={:?} source={source_uid} targets={condition_targets:?} skill={} random_roll={:?} buff_added={}/{}x{} on {} matched={matched}",
            condition.opcode,
            condition.type_name,
            condition.raw_args,
            context.active_skill_id,
            context.condition_random_roll,
            context.added_buff_id,
            context.added_buff_type_id,
            context.added_buff_amount,
            context.added_buff_target_uid,
        );
    }
    matched
}

fn action_owner(source_uid: i64, context: TargetContext) -> i64 {
    if context.active_skill_source_uid != 0 {
        context.active_skill_source_uid
    } else {
        source_uid
    }
}

fn card_ultimate(
    source_uid: i64,
    pool: &TargetPool,
    managers: Option<&BattleManagers>,
    context: TargetContext,
) -> bool {
    pool.entity(action_owner(source_uid, context))
        .is_some_and(|source| {
            managers.is_some_and(|managers| {
                crate::engine::mechanic::card::CardMechanic.is_ultimate_skill(
                    managers,
                    context.active_skill_id,
                    source,
                )
            })
        })
}

fn card_anchor_matches(
    source_uid: i64,
    active_card_index: i32,
    order: super::card::AnchorOrder,
    managers: &BattleManagers,
) -> bool {
    let Some(hand_index) = managers
        .card
        .played()
        .iter()
        .find(|played| played.caster_uid == source_uid && played.card_index == active_card_index)
        .and_then(|played| played.hand_index)
    else {
        return false;
    };
    let Some(anchor_index) = managers.card.committed_hand().iter().position(|card| {
        card.uid == Some(source_uid)
            && card.card_type == Some(sonettobuf::card_info::CardType::Skill3 as i32)
    }) else {
        return false;
    };

    match order {
        super::card::AnchorOrder::Earlier => hand_index < anchor_index,
        super::card::AnchorOrder::Later => hand_index > anchor_index,
    }
}

fn conduit_counter_count(
    source_uid: i64,
    kind: crate::engine::manager::conduit::ConduitCounterKind,
    divisor: i32,
    max_count: i32,
    managers: &BattleManagers,
    pool: &TargetPool,
) -> i32 {
    let Some(team) = pool.team_type(source_uid) else {
        return 0;
    };
    (managers.conduit.counter(team, kind) / divisor.max(1)).clamp(0, max_count.max(0))
}

fn current_magic_circle_id(
    source_uid: i64,
    managers: Option<&BattleManagers>,
    pool: &TargetPool,
    context: TargetContext,
) -> i32 {
    managers
        .and_then(|managers| {
            let team = if pool.source_is_attacker(source_uid) {
                1
            } else {
                2
            };
            managers.field.get(team)
        })
        .map(|field| field.definition.field_id)
        .unwrap_or(context.magic_circle_id)
}

fn active_skill_has_real_source(pool: &TargetPool, context: TargetContext) -> bool {
    context.active_skill_source_uid == 0
        || pool
            .entities()
            .any(|entity| entity.uid == context.active_skill_source_uid)
}

fn specific_skill_matches(
    source_uid: i64,
    group: i32,
    rank: i32,
    pool: &TargetPool,
    context: TargetContext,
) -> bool {
    pool.entity(source_uid).is_some_and(|source| {
        let matches_group = match group {
            0 | 4 | 5 => {
                source.skill_group1.contains(&context.active_skill_id)
                    || source.skill_group2.contains(&context.active_skill_id)
            }
            1 => source.skill_group1.contains(&context.active_skill_id),
            2 => source.skill_group2.contains(&context.active_skill_id),
            3 => source.ex_skill == context.active_skill_id,
            _ => false,
        };
        matches_group && (rank <= 0 || context.active_skill_rank == rank)
    })
}

fn has_master_halo(uid: i64, managers: Option<&BattleManagers>) -> bool {
    managers.is_some_and(|managers| managers.buff.has_force_field(uid))
}

fn extra_action_kind_matches(actual: i32, expected: &[i32]) -> bool {
    expected.contains(&actual) || (matches!(actual, 2 | 3) && expected.contains(&1))
}

fn compare_value(value: i32, compare: ConditionCompare, expected: i32) -> bool {
    match compare {
        ConditionCompare::Equal => value == expected,
        ConditionCompare::NotEqual => value != expected,
        ConditionCompare::GreaterThan => value > expected,
        ConditionCompare::GreaterThanOrEqual => value >= expected,
        ConditionCompare::LessThan => value < expected,
        ConditionCompare::LessThanOrEqual => value <= expected,
    }
}

pub(crate) fn compare_resource(value: i32, compare_code: i32, expected: i32) -> bool {
    match compare_code {
        1 => value >= expected,
        2 => value <= expected,
        3 => value == expected,
        4 => value != expected,
        6 => value <= expected,
        7 => value < expected,
        _ => false,
    }
}

#[cfg(test)]
mod tests;
