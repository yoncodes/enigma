use crate::engine::{
    event::kind::EventKind,
    manager::{BattleManagers, card::CardManager},
    runtime::determinism::RoundDeterminism,
    skill::{
        behavior::{classify::BehaviorKind, queue_rank_by_tag, registry as behaviors},
        buff_act::{registry as buff_acts, registry::BuffActKind},
        condition::{ParsedConditionKind, none::NoneMode},
        effect::{SkillEffectCatalog, SkillEffectSlot, catalog::SkillEffectTag},
        subscriber,
        target::{TargetContext, TargetPool, TargetResolver, request::SOURCE_TARGET_CODE, resolve},
    },
};

use super::Candidate;

pub(super) fn available(
    candidate: &Candidate,
    cards: &CardManager,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    pool: &TargetPool,
    determinism: &RoundDeterminism,
) -> bool {
    let owner = candidate.source_uid;
    let skill = candidate.skill_id;
    let config = managers.catalog();
    if !candidate.first_for_owner
        || candidate.free
        || candidate.strength != Some((0, 1))
        || candidate.chosen_skill_id.is_some()
        || !cards
            .visible_card(candidate.card_index)
            .is_some_and(|card| {
                !card.temp_card.unwrap_or_default() && card.energy.unwrap_or_default() == 0
            })
        || config.skill_rank(skill) != 1
    {
        return false;
    }
    let Some(base) = grant(skill, catalog) else {
        return false;
    };
    let Some(next) = cards.next_rank(owner, skill) else {
        return false;
    };
    let Some(upgraded) = grant(next, catalog) else {
        return false;
    };
    let [buff] = base.behavior.args.as_slice() else {
        return false;
    };
    let [upgraded_buff] = upgraded.behavior.args.as_slice() else {
        return false;
    };
    let buff = *buff;
    let family = config.buff_type_id(buff);
    if base.target.code != SOURCE_TARGET_CODE
        || upgraded.target.code == 0
        || resolve::uses_rng(upgraded.target.code)
        || family <= 0
        || config.skill_rank(next) != 2
        || config.buff_type_id(*upgraded_buff) != family
        || managers.buff.has_active_buff_id_or_type(owner, family)
        || config
            .game_data()
            .skill_buff
            .get(buff)
            .is_none_or(|row| row.during_time <= 0)
        || config.buff_has_effect_count(buff)
        || !managers
            .buff
            .definition_features(buff)
            .iter()
            .any(|feature| {
                feature
                    .act_id()
                    .and_then(|id| buff_acts::find(id, &feature.act_type))
                    .is_some_and(|definition| {
                        definition.kind == BuffActKind::UseDamageSkillAddToTarget
                    })
            })
    {
        return false;
    }

    let owner_slot = pool.skill_slot(managers, owner, skill);
    let subscriptions = subscriber::for_compiled_owner_skill_event(
        pool,
        managers,
        catalog,
        EventKind::ActionQueueSubmitted,
        owner,
    );
    if !subscriptions.is_ok_and(|subscriptions| {
        subscriptions.skills.into_iter().any(|listener| {
            let Some(slot) = listener
                .slot_index
                .and_then(|index| catalog.get(listener.skill_id)?.slots.get(index))
            else {
                return false;
            };
            let ([tag, skill_slot], [condition]) =
                (slot.behavior.args.as_slice(), slot.conditions.as_slice())
            else {
                return false;
            };
            *skill_slot == owner_slot
                && condition.kind == ParsedConditionKind::None(NoneMode::ActionQueueCommitted)
                && condition.raw_args.is_empty()
                && slot.limit == 0
                && slot.round_limit == 0
                && behaviors::find(&slot.behavior).is_some_and(|definition| {
                    definition.kind == BehaviorKind::AddCardRankByEffectTag
                })
                && queue_rank_by_tag::matches_card(config, skill, *tag)
                && {
                    let mut matching = cards.played().iter().filter(|card| {
                        queue_rank_by_tag::matches_card(config, card.skill_id, *tag)
                    });
                    matching.next().is_some_and(|card| {
                        !card.card.temp_card.unwrap_or_default()
                            && card.card.energy.unwrap_or_default() == 0
                    }) && matching.next().is_none()
                }
        })
    }) {
        return false;
    }

    let targets = TargetResolver::resolve_with_managers_and_context(
        &upgraded.target,
        next,
        owner,
        pool,
        &mut determinism.clone(),
        Some(managers),
        TargetContext::default(),
    );
    targets.iter().any(|&uid| {
        uid != owner
            && pool.team_type(uid) == pool.team_type(owner)
            && !managers.buff.has_active_buff_id_or_type(uid, family)
    })
}

fn grant(skill: i32, catalog: &SkillEffectCatalog) -> Option<&SkillEffectSlot> {
    let [slot] = catalog.get(skill)?.slots.as_slice() else {
        return None;
    };
    let [condition] = slot.conditions.as_slice() else {
        return None;
    };
    (catalog.issues(skill).is_empty()
        && catalog.effect_tag(skill) == SkillEffectTag::Buff as i32
        && catalog.damage_rate(skill) == 0
        && !slot.target_from_condition
        && condition.kind == ParsedConditionKind::None(NoneMode::SkillActionAfterDamage)
        && condition.raw_args.is_empty()
        && behaviors::find(&slot.behavior)
            .is_some_and(|definition| definition.kind == BehaviorKind::AddBuff))
    .then_some(slot)
}
