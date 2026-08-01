use crate::engine::{
    entity::attr::AttrId,
    manager::{
        BattleManagers,
        conduit::ConduitCommand,
        ex_point::{ExPointCommand, ExPointSet},
    },
    runtime::determinism::RoundDeterminism,
    skill::{
        action::{SkillModifiers, SkillRateModifier},
        behavior::{
            AttackModifierContext, BehaviorOpContext, classify::BehaviorKind,
            registry::BehaviorHandler,
        },
        condition::{conditions_fire_count, registry as condition_registry, satisfied_conditions},
        effect::{ParsedBehavior, SkillEffectCatalog, SkillEffectSlot},
        rule::output::{BattleCommand, RuleOp},
        target::{TargetContext, TargetPool, TargetResolver},
    },
};

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    fn emit_ops(
        context: BehaviorOpContext<'_>,
        behavior: &ParsedBehavior,
    ) -> Option<Vec<crate::engine::skill::rule::output::RuleOp>> {
        if behavior.spec.kind == BehaviorKind::ConduitPowerUp {
            return conduit_power_up_ops(context, behavior);
        }
        if behavior.spec.kind == BehaviorKind::ConduitRateByConsumedPower {
            let [_, _, rate, threshold, _, _, threshold_rate] = behavior.args.as_slice() else {
                return None;
            };
            let consumed = context
                .managers
                .conduit
                .consumed_for_skill(context.source_uid, context.active_skill_id)?;
            let bonus = if *threshold <= consumed {
                *threshold_rate
            } else {
                0
            };
            let delta = consumed.saturating_mul(rate.saturating_add(bonus));
            if delta != 0 {
                context.modifiers.rates.push(SkillRateModifier::fixed(
                    0,
                    behavior.spec.key.opcode,
                    delta,
                    true,
                ));
            }
            return Some(vec![
                crate::engine::skill::rule::output::RuleOp::EffectMarker {
                    target_uid: context.source_uid,
                    effect_type: sonettobuf::effect_type_enum::EffectType::Twinsupcount as i32,
                    effect_num: consumed,
                    config_effect: behavior.spec.key.opcode,
                    reserve_id: None,
                    reserve_str: None,
                },
            ]);
        }
        if behavior.spec.kind == BehaviorKind::CrystalAddSkillRate {
            let [all_rate, raw_limit, focus_rate, buff_id, crystal_type] = behavior.args.as_slice()
            else {
                return None;
            };
            let crystal_count = crystal_count(context.managers, context.source_uid, *crystal_type);
            let gauge_key = crate::engine::mechanic::lingering_glow::key(context.source_team);
            if *all_rate != 0 && crystal_count > 0 {
                context.modifiers.rates.push(SkillRateModifier::new(
                    0,
                    behavior.spec.key.opcode,
                    crate::engine::skill::action::SkillRateAmount::gauge_raw(
                        gauge_key,
                        *raw_limit,
                        *all_rate,
                        crystal_count,
                        1000,
                    ),
                    crystal_rate_career_scaled(behavior.spec.kind, false),
                ));
            }
            if *focus_rate != 0
                && crystal_count > 0
                && let Some(target_uid) = context
                    .pool
                    .enemies(context.source_uid, false)
                    .iter()
                    .map(|enemy| enemy.uid)
                    .max_by_key(|uid| context.managers.buff.buff_id_amount(*uid, *buff_id))
            {
                context.modifiers.rates.push(SkillRateModifier::new(
                    target_uid,
                    behavior.spec.key.opcode,
                    crate::engine::skill::action::SkillRateAmount::gauge_raw(
                        gauge_key,
                        *raw_limit,
                        *focus_rate,
                        crystal_count,
                        1000,
                    ),
                    crystal_rate_career_scaled(behavior.spec.kind, true),
                ));
            }
            return Some(Vec::new());
        }
        if behavior.spec.kind == BehaviorKind::CrystalAddCardRank {
            let (all_rate, focus_rate) = crystal_fixed_skill_rates(&behavior.args)?;
            if all_rate != 0 {
                context.modifiers.rates.push(SkillRateModifier::fixed(
                    0,
                    behavior.spec.key.opcode,
                    all_rate,
                    crystal_rate_career_scaled(behavior.spec.kind, false),
                ));
            }
            if focus_rate != 0 && context.target.runtime_target_uid != 0 {
                context.modifiers.rates.push(SkillRateModifier::fixed(
                    context.target.runtime_target_uid,
                    behavior.spec.key.opcode,
                    focus_rate,
                    crystal_rate_career_scaled(behavior.spec.kind, true),
                ));
            }
            return Some(Vec::new());
        }
        if behavior.spec.kind == BehaviorKind::HeatScaleAddSkillRate {
            let current = context
                .managers
                .gauge
                .get(crate::engine::mechanic::lingering_glow::key(
                    context.source_team,
                ))
                .map(|state| state.current)
                .unwrap_or_default();
            let delta = heat_scale_skill_rate(current, &behavior.args);
            if context.active_skill_id != 0 && delta != 0 {
                context.modifiers.rates.push(SkillRateModifier::fixed(
                    context.target_uid,
                    behavior.spec.key.opcode,
                    delta,
                    true,
                ));
            }
            return Some(Vec::new());
        }
        if behavior.spec.kind == BehaviorKind::AddSkillRateByTargetCount {
            let rate = behavior.arg(0)?;
            let enemy_count = i32::try_from(
                context
                    .pool
                    .enemies(context.source_uid, false)
                    .iter()
                    .filter(|enemy| context.managers.hp.current(enemy.uid) > 0)
                    .count(),
            )
            .ok()?;
            if context.active_skill_id != 0 && rate != 0 && enemy_count > 0 {
                context.modifiers.rates.push(SkillRateModifier::fixed(
                    0,
                    behavior.spec.key.opcode,
                    rate.saturating_mul(enemy_count),
                    true,
                ));
            }
            return Some(Vec::new());
        }
        if behavior.spec.kind == BehaviorKind::SkillRateUp2 {
            let target_uid = context.target.runtime_target_uid;
            let delta = status_skill_rate(&context.managers.buff, target_uid, behavior);
            if context.active_skill_id != 0 && target_uid != 0 && delta != 0 {
                context.modifiers.rates.push(SkillRateModifier::fixed(
                    target_uid,
                    behavior.spec.key.opcode,
                    delta,
                    true,
                ));
            }
            return Some(Vec::new());
        }
        let additional_moxie = context.target.additional_moxie;
        emit(
            context.modifiers,
            context.source_uid,
            context.target_uid,
            Some(context.managers),
            context.active_skill_id,
            additional_moxie,
            behavior,
        )
        .then(Vec::new)
    }

    fn collect_attack_modifier(
        context: AttackModifierContext<'_>,
        behavior: &ParsedBehavior,
    ) -> bool {
        if behavior.spec.kind != BehaviorKind::BulletCritRateAlter {
            return Self::emit_ops(context.operation, behavior).is_some();
        }
        let [base_rate, bonus_rate, max_count] = behavior.args.as_slice() else {
            return false;
        };
        let Some(group_ids) =
            context
                .conditions
                .iter()
                .find_map(|condition| match &condition.kind {
                    crate::engine::skill::condition::ParsedConditionKind::BuffGroup(ids) => {
                        Some(ids.as_slice())
                    }
                    _ => None,
                })
        else {
            return false;
        };
        let count = context
            .operation
            .managers
            .buff
            .buff_group_type_count(context.operation.source_uid, group_ids)
            .min(*max_count);
        context.operation.modifiers.excess_crit_conversion_rate += base_rate + bonus_rate * count;
        true
    }
}

pub(super) fn supports_conduit_rate(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [_, _, rate, threshold, _, _, threshold_rate]
            if *rate >= 0 && *threshold > 0 && *threshold_rate >= 0
    )
}

pub(super) fn supports_conduit_power_up(behavior: &ParsedBehavior) -> bool {
    conduit_power_up_args(behavior).is_some()
}

fn conduit_power_up_ops(
    context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let args = conduit_power_up_args(behavior)?;
    let spent = args
        .power_ids
        .iter()
        .map(|power_id| {
            context
                .managers
                .conduit
                .power(context.source_team, *power_id)
        })
        .fold(0i32, i32::saturating_add);
    let mut rate = spent.saturating_mul(args.rate_per_power);
    for (threshold, bonus) in args.thresholds.into_iter().zip(args.threshold_bonuses) {
        if spent >= threshold {
            rate = rate.saturating_add(bonus);
        }
    }
    if rate != 0 {
        context.modifiers.rates.push(SkillRateModifier::fixed(
            0,
            behavior.spec.key.opcode,
            rate,
            true,
        ));
    }
    if spent >= args.thresholds[0] {
        context.modifiers.attack_attributes.push(args.critical_rate);
    }
    if spent >= args.thresholds[1] {
        context.modifiers.attack_attributes.push(args.penetration);
    }
    if spent >= args.thresholds[2] {
        context.modifiers.excess_crit_conversion_rate = context
            .modifiers
            .excess_crit_conversion_rate
            .saturating_add(args.excess_crit_conversion);
    }

    let origin = super::command_origin(behavior)?;
    Some(vec![
        RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Set(ExPointSet {
            origin,
            source_uid: context.source_uid,
            target_uid: context.source_uid,
            value: 0,
            config_effect: behavior.spec.key.opcode,
            effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
        }))),
        RuleOp::Command(BattleCommand::Conduit(ConduitCommand::ClearPowers {
            origin,
            source_uid: context.source_uid,
            team: context.source_team,
            skill_id: context.active_skill_id,
            power_ids: args.power_ids,
        })),
        RuleOp::EffectMarker {
            target_uid: context.source_uid,
            effect_type: sonettobuf::effect_type_enum::EffectType::Twinspowerupcount as i32,
            effect_num: spent,
            config_effect: behavior.spec.key.opcode,
            reserve_id: None,
            reserve_str: None,
        },
    ])
}

#[derive(Clone, Copy)]
struct ConduitPowerUpArgs {
    power_ids: [i32; 2],
    rate_per_power: i32,
    thresholds: [i32; 3],
    threshold_bonuses: [i32; 3],
    critical_rate: (AttrId, i32),
    penetration: (AttrId, i32),
    excess_crit_conversion: i32,
}

fn conduit_power_up_args(behavior: &ParsedBehavior) -> Option<ConduitPowerUpArgs> {
    let power_ids = behavior.arg_list(0)?;
    let [0, power_a, power_b] = power_ids.as_slice() else {
        return None;
    };
    let args = ConduitPowerUpArgs {
        power_ids: [*power_a, *power_b],
        rate_per_power: behavior.arg(1)?,
        thresholds: [behavior.arg(2)?, behavior.arg(3)?, behavior.arg(4)?],
        threshold_bonuses: [behavior.arg(5)?, behavior.arg(6)?, behavior.arg(7)?],
        critical_rate: (AttrId::from_raw(behavior.arg(8)?)?, behavior.arg(9)?),
        penetration: (AttrId::from_raw(behavior.arg(10)?)?, behavior.arg(11)?),
        excess_crit_conversion: behavior.arg(12)?,
    };
    (args.power_ids.iter().all(|id| *id > 0)
        && args.rate_per_power >= 0
        && args.thresholds.iter().all(|threshold| *threshold > 0)
        && args.thresholds.windows(2).all(|pair| pair[0] < pair[1])
        && args.threshold_bonuses.iter().all(|bonus| *bonus >= 0)
        && args.critical_rate.0 == AttrId::CriticalRate
        && args.penetration.0 == AttrId::Penetration
        && args.excess_crit_conversion >= 0)
        .then_some(args)
}

use crate::engine::skill::behavior::magic_circle;

#[derive(Clone, Copy)]
pub struct RateRuntime<'a> {
    pub effects: &'a SkillEffectCatalog,
    pub managers: &'a BattleManagers,
    pub pool: &'a TargetPool,
    pub context: TargetContext,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModifierSide {
    Source,
    IncomingTarget,
}

fn collect_attack_modifier_slot(
    modifiers: &mut crate::engine::skill::action::SkillModifiers,
    owner_uid: i64,
    active_skill_id: i32,
    slot: &SkillEffectSlot,
    runtime: RateRuntime<'_>,
    determinism: &mut RoundDeterminism,
    side: ModifierSide,
) -> bool {
    let RateRuntime {
        managers,
        pool,
        context,
        ..
    } = runtime;
    let incoming = condition_registry::attack_modifier_side(&slot.conditions)
        == Some(condition_registry::AttackModifierSide::IncomingTarget);
    if incoming != (side == ModifierSide::IncomingTarget) {
        return false;
    }
    let Some(definition) = super::registry::find(&slot.behavior) else {
        return false;
    };
    let Some(collect) = definition.collect_attack_modifier else {
        return false;
    };
    let Ok(route) = slot.compiled_route.as_ref() else {
        return false;
    };
    let immediate_drivers = route
        .branches
        .iter()
        .filter_map(|branch| match branch.driver {
            Some(crate::engine::skill::rule::route::ConditionDriver::Trigger(trigger))
                if trigger.event == crate::engine::event::kind::EventKind::SkillAction
                    && matches!(
                        trigger.phase,
                        None | Some(crate::engine::skill::action::SkillPhase::Immediate)
                    ) =>
            {
                Some(trigger.key)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let has_static_branch = route.branches.iter().any(|branch| branch.driver.is_none());
    if !has_static_branch && immediate_drivers.is_empty() {
        return false;
    }
    let mut condition_sets = has_static_branch
        .then(|| slot.conditions.clone())
        .into_iter()
        .collect::<Vec<_>>();
    condition_sets.extend(
        immediate_drivers
            .into_iter()
            .map(|key| satisfied_conditions(&slot.conditions, key)),
    );
    let Some(source_team) = pool.team_type(owner_uid) else {
        return false;
    };
    for conditions in condition_sets {
        let condition_targets = TargetResolver::resolve_with_managers_and_context(
            &slot.condition_target,
            active_skill_id,
            owner_uid,
            pool,
            determinism,
            Some(managers),
            context,
        );
        let fire_count = conditions_fire_count(
            &conditions,
            owner_uid,
            &condition_targets,
            Some(managers),
            pool,
            context,
        );
        if fire_count <= 0 {
            continue;
        }
        let targets = TargetResolver::resolve_with_managers_and_context(
            &slot.target,
            active_skill_id,
            owner_uid,
            pool,
            determinism,
            Some(managers),
            context,
        );
        let current_hit_target = context
            .hit_target_uid
            .ne(&0)
            .then_some(context.hit_target_uid)
            .or_else(|| (context.runtime_target_uid != 0).then_some(context.runtime_target_uid));
        let targets_owner = targets.contains(&owner_uid);
        let targets_current_hit =
            current_hit_target.is_some_and(|target_uid| targets.contains(&target_uid));
        let applies_to_current_attack =
            targets_owner || (side == ModifierSide::Source && targets_current_hit);
        if !applies_to_current_attack {
            continue;
        }
        let modifier_target_uid = if side == ModifierSide::IncomingTarget
            || (side == ModifierSide::Source && targets_current_hit && !targets_owner)
        {
            current_hit_target.unwrap_or_default()
        } else {
            0
        };
        let mut collected = false;
        for _ in 0..fire_count {
            let mut target_context = context;
            collected |= collect(
                AttackModifierContext {
                    operation: BehaviorOpContext {
                        source_uid: owner_uid,
                        source_team,
                        target_uid: modifier_target_uid,
                        active_skill_id,
                        transfer_count: 1,
                        event: None,
                        managers,
                        pool,
                        determinism,
                        modifiers,
                        target: &mut target_context,
                    },
                    conditions: &conditions,
                },
                &slot.behavior,
            );
        }
        if collected {
            modifiers.consume_team_injury_count_round = modifiers
                .consume_team_injury_count_round
                .or_else(|| team_injury_count_key(&conditions));
            return true;
        }
    }
    false
}

fn team_injury_count_key(
    conditions: &[crate::engine::skill::condition::ParsedCondition],
) -> Option<crate::engine::skill::rule::DefinitionKey> {
    conditions
        .iter()
        .find_map(|condition| match &condition.kind {
            crate::engine::skill::condition::ParsedConditionKind::TeamInjuryCountRound {
                ..
            } => crate::engine::skill::condition::registry::find_key(
                condition.opcode,
                &condition.type_name,
            )
            .map(|definition| definition.key),
            crate::engine::skill::condition::ParsedConditionKind::Any(groups) => {
                groups.iter().find_map(|group| team_injury_count_key(group))
            }
            _ => None,
        })
}

pub fn emit_passive_attack_attributes(
    modifiers: &mut crate::engine::skill::action::SkillModifiers,
    source_uid: i64,
    active_skill_id: i32,
    passive_skills: &[i32],
    runtime: RateRuntime<'_>,
    determinism: &mut RoundDeterminism,
) {
    let RateRuntime {
        effects,
        managers,
        pool,
        context,
    } = runtime;
    let mut passive_skills = managers
        .entity
        .passive_override(source_uid)
        .unwrap_or(passive_skills)
        .to_vec();
    for link in managers.buff.passive_skill_links_for(source_uid) {
        if !passive_skills.contains(&link.skill_id) {
            passive_skills.push(link.skill_id);
        }
    }
    for skill_id in magic_circle::active_self_skills(source_uid, managers, pool) {
        if !passive_skills.contains(&skill_id) {
            passive_skills.push(skill_id);
        }
    }
    for passive_skill in &passive_skills {
        let Some(effect) = effects.get(*passive_skill) else {
            continue;
        };
        for slot in &effect.slots {
            let attribute_count_before = modifiers.attack_attributes.len();
            let career_ratio_before = modifiers.career_ratio_bonus;
            collect_attack_modifier_slot(
                modifiers,
                source_uid,
                active_skill_id,
                slot,
                runtime,
                determinism,
                ModifierSide::Source,
            );
            if crate::engine::diagnostics::enabled(crate::engine::diagnostics::TraceArea::Damage)
                && (modifiers.attack_attributes.len() > attribute_count_before
                    || modifiers.career_ratio_bonus != career_ratio_before)
            {
                eprintln!(
                    "attack modifier owner={source_uid} passive={passive_skill} active_skill={active_skill_id} opcode={} attributes={:?} career_ratio_bonus={}",
                    slot.behavior.spec.key.opcode,
                    &modifiers.attack_attributes[attribute_count_before..],
                    modifiers.career_ratio_bonus - career_ratio_before,
                );
            }
        }
    }

    magic_circle::emit_attack_attributes(
        &mut modifiers.attack_attributes,
        source_uid,
        active_skill_id,
        effects,
        managers,
        pool,
        determinism,
        context,
    );
}

pub(crate) fn incoming_target_attack_modifiers(
    source_uid: i64,
    target_uid: i64,
    active_skill_id: i32,
    runtime: RateRuntime<'_>,
    determinism: &mut RoundDeterminism,
) -> SkillModifiers {
    if source_uid == crate::engine::manager::emitter::UID {
        return SkillModifiers::default();
    }
    let RateRuntime {
        effects,
        managers,
        pool,
        mut context,
    } = runtime;
    if pool.entity(target_uid).is_none() || pool.entity(source_uid).is_none() {
        return SkillModifiers::default();
    }
    context.hit_source_uid = source_uid;
    context.hit_target_uid = target_uid;
    context.runtime_target_uid = source_uid;
    let mut passive_skills = managers
        .entity
        .passive_override(target_uid)
        .or_else(|| {
            pool.entity(target_uid)
                .map(|target| target.passive_skills.as_slice())
        })
        .unwrap_or_default()
        .to_vec();
    for link in managers.buff.passive_skill_links_for(target_uid) {
        if !passive_skills.contains(&link.skill_id) {
            passive_skills.push(link.skill_id);
        }
    }
    if crate::engine::diagnostics::enabled(crate::engine::diagnostics::TraceArea::Damage) {
        eprintln!(
            "incoming modifier scan owner={target_uid} active_skill={active_skill_id} passives={passive_skills:?}"
        );
    }
    let mut modifiers = SkillModifiers::default();
    for passive_skill in passive_skills {
        let Some(effect) = effects.get(passive_skill) else {
            continue;
        };
        for slot in &effect.slots {
            let attribute_count_before = modifiers.attack_attributes.len();
            let career_ratio_before = modifiers.career_ratio_bonus;
            collect_attack_modifier_slot(
                &mut modifiers,
                target_uid,
                active_skill_id,
                slot,
                RateRuntime {
                    effects,
                    managers,
                    pool,
                    context,
                },
                determinism,
                ModifierSide::IncomingTarget,
            );
            if crate::engine::diagnostics::enabled(crate::engine::diagnostics::TraceArea::Damage)
                && (modifiers.attack_attributes.len() > attribute_count_before
                    || modifiers.career_ratio_bonus != career_ratio_before)
            {
                eprintln!(
                    "incoming modifier owner={target_uid} passive={passive_skill} active_skill={active_skill_id} opcode={} attributes={:?} career_ratio_bonus={}",
                    slot.behavior.spec.key.opcode,
                    &modifiers.attack_attributes[attribute_count_before..],
                    modifiers.career_ratio_bonus - career_ratio_before,
                );
            }
        }
    }
    modifiers
}

pub fn emit(
    modifiers: &mut crate::engine::skill::action::SkillModifiers,
    source_uid: i64,
    target_uid: i64,
    managers: Option<&BattleManagers>,
    active_skill_id: i32,
    additional_moxie: i32,
    behavior: &ParsedBehavior,
) -> bool {
    let opcode = behavior.spec.key.opcode;
    let amount = match (opcode, behavior.spec.kind) {
        (40001, BehaviorKind::CritRateAlter) | (100023 | 60228, BehaviorKind::CritRateAlter2) => {
            modifiers.excess_crit_conversion_rate = modifiers
                .excess_crit_conversion_rate
                .saturating_add(behavior.arg(0).unwrap_or_default());
            return true;
        }
        (10004, BehaviorKind::AttrFix) => {
            let [attr_id, delta] = behavior.args.as_slice() else {
                return true;
            };
            let Some(attr_id) = AttrId::from_raw(*attr_id) else {
                return true;
            };
            if active_skill_id != 0 && *delta != 0 {
                modifiers.attack_attributes.push((attr_id, *delta));
            }
            return true;
        }
        (10001, BehaviorKind::SkillRateUp) => behavior.args.first().copied().unwrap_or_default(),
        (10002, BehaviorKind::SkillRateUp1) => managers
            .map(|managers| status_skill_rate(&managers.buff, source_uid, behavior))
            .unwrap_or_default(),
        (60067, BehaviorKind::SkillRateUpCardLevel) => managers
            .map(|managers| card_rank_skill_rate(&managers.card, behavior))
            .unwrap_or_default(),
        (60234, BehaviorKind::AddSkillRateByTargetCount) => behavior.arg(0).unwrap_or_default(),
        (60182, BehaviorKind::SkillRateUpBySelfBuffType) => {
            let [buff_type, rate] = behavior.args.as_slice() else {
                return true;
            };
            let count = managers
                .map(|managers| managers.buff.buff_type_amount(source_uid, *buff_type))
                .unwrap_or_default();
            rate * count
        }
        (60174, BehaviorKind::ConsumeExPointAddAttr) => {
            let [
                attr_id,
                per_point,
                minimum,
                double_at,
                multiplier_attr,
                multiplier,
            ] = behavior.args.as_slice()
            else {
                return true;
            };
            let Some(attr_id) = AttrId::from_raw(*attr_id) else {
                return true;
            };
            if additional_moxie < *minimum {
                return true;
            }
            let mut delta = per_point * additional_moxie;
            if additional_moxie >= *double_at
                && AttrId::from_raw(*multiplier_attr) == Some(AttrId::UltimateMightMultiplier)
            {
                delta = delta * (1000 + multiplier) / 1000;
            }
            if active_skill_id != 0 && delta != 0 {
                modifiers.attack_attributes.push((attr_id, delta));
            }
            return true;
        }
        _ => return false,
    };

    if active_skill_id != 0 && amount != 0 {
        let modifier_target = if target_uid == source_uid
            || matches!(
                behavior.spec.kind,
                BehaviorKind::SkillRateUp1 | BehaviorKind::SkillRateUpCardLevel
            ) {
            0
        } else {
            target_uid
        };
        modifiers.rates.push(SkillRateModifier::fixed(
            modifier_target,
            opcode,
            amount,
            true,
        ));
    }
    true
}

pub(super) fn supports_attr_fix(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [attr_id, _] if AttrId::from_raw(*attr_id).is_some())
}

pub(super) fn supports_self_buff_type_rate(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [buff_type, rate] if *buff_type > 0 && *rate > 0)
}

pub(super) fn supports_target_count_rate(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [rate] if *rate > 0)
}

pub(super) fn supports_status_skill_rate(behavior: &ParsedBehavior) -> bool {
    behavior.arg(0).is_some_and(|rate| rate > 0)
        && behavior.arg(1).is_some_and(|limit| limit > 0)
        && behavior
            .arg_list(2)
            .is_some_and(|statuses| statuses.iter().all(|status| *status > 0))
}

pub(super) fn supports_card_rank_skill_rate(behavior: &ParsedBehavior) -> bool {
    let Some(ranks) = behavior.arg_list(0) else {
        return false;
    };
    let Some(rates) = behavior.arg_list(1) else {
        return false;
    };
    !ranks.is_empty()
        && ranks.len() == rates.len()
        && ranks.iter().all(|rank| (1..=3).contains(rank))
        && rates.iter().all(|rate| *rate > 0)
        && behavior.arg(2).is_some_and(|cap| cap > 0)
}

pub(super) fn supports_crit_rate_alter(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [rate] if *rate >= 0)
}

pub(super) fn supports_heat_scale_rate(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [divisor, rate, limit]
        if *divisor > 0 && *rate > 0 && *limit > 0)
}

fn heat_scale_skill_rate(current: i32, args: &[i32]) -> i32 {
    let [divisor, rate, limit] = args else {
        return 0;
    };
    if *divisor <= 0 || *rate <= 0 || *limit <= 0 {
        return 0;
    }
    (i64::from(current.clamp(0, *limit)) * i64::from(*rate) * 1000 / i64::from(*divisor))
        .try_into()
        .unwrap_or(i32::MAX)
}

fn status_skill_rate(
    buffs: &crate::engine::manager::buff::BuffManager,
    source_uid: i64,
    behavior: &ParsedBehavior,
) -> i32 {
    let Some(rate) = behavior.arg(0) else {
        return 0;
    };
    let Some(limit) = behavior.arg(1) else {
        return 0;
    };
    let Some(statuses) = behavior.arg_list(2) else {
        return 0;
    };
    buffs
        .buff_status_count(source_uid, &statuses)
        .min(limit)
        .saturating_mul(rate)
}

fn card_rank_skill_rate(
    cards: &crate::engine::manager::card::CardManager,
    behavior: &ParsedBehavior,
) -> i32 {
    let Some(ranks) = behavior.arg_list(0) else {
        return 0;
    };
    let Some(rates) = behavior.arg_list(1) else {
        return 0;
    };
    let Some(cap) = behavior.arg(2) else {
        return 0;
    };
    if ranks.len() != rates.len() {
        return 0;
    }
    let total = cards
        .resolving_ranks()
        .filter_map(|rank| {
            ranks
                .iter()
                .position(|configured| *configured == rank)
                .map(|index| rates[index])
        })
        .fold(0_i32, i32::saturating_add);
    total.min(cap)
}

fn crystal_count(managers: &BattleManagers, source_uid: i64, crystal_type: i32) -> i32 {
    crate::engine::manager::emanation::EmanationKind::from_id(crystal_type)
        .map(|kind| managers.emanation.count(source_uid, kind))
        .unwrap_or_default()
        .max(0)
}

fn crystal_rate_career_scaled(kind: BehaviorKind, _focus: bool) -> bool {
    matches!(
        kind,
        BehaviorKind::CrystalAddSkillRate | BehaviorKind::CrystalAddCardRank
    )
}

fn crystal_fixed_skill_rates(args: &[i32]) -> Option<(i32, i32)> {
    let [all_rate, focus_rate, _] = args else {
        return None;
    };
    Some((*all_rate, *focus_rate))
}

#[cfg(test)]
mod test;
