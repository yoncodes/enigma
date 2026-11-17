use crate::engine::skill::{
    behavior::{classify::BehaviorKind, registry},
    condition::{ParsedConditionKind, buff::BuffConditionMode, none::NoneMode},
    target::{TargetRequest, TargetResolver},
};

use super::SkillEffectSlot;

pub(crate) fn is_cast_slot(slot: &SkillEffectSlot) -> bool {
    slot.behavior.slot == 1
        && slot.target == TargetRequest::default()
        && !slot.target_from_condition
        && slot.behavior.raw_args.is_empty()
        && registry::find(&slot.behavior)
            .is_some_and(|definition| definition.kind == BehaviorKind::DirectUseBigSkill)
}

pub(super) fn bind_reward(
    slots: &mut [SkillEffectSlot],
    logic_target: &TargetRequest,
    raw_reward_target: &str,
) {
    if raw_reward_target.trim() != "999"
        || logic_target.raw.len() != 1
        || !TargetResolver::is_highest_moxie(logic_target)
    {
        return;
    }
    let [cast, reward] = slots else {
        return;
    };
    let [trigger] = cast.conditions.as_slice() else {
        return;
    };
    if !is_cast_slot(cast)
        || cast.behavior.raw_arity != 0
        || !matches!(
            trigger.kind,
            ParsedConditionKind::None(NoneMode::SkillActionAfterHit)
        )
        || !trigger.raw_args.is_empty()
    {
        return;
    }
    let Ok(phases) = cast.active_phases() else {
        return;
    };
    let [phase] = phases.as_slice() else {
        return;
    };

    let [predicate] = reward.conditions.as_slice() else {
        return;
    };
    if reward.behavior.slot == 2
        && reward.behavior.raw_arity == 1
        && reward.behavior.arg(0) == Some(1)
        && registry::find(&reward.behavior)
            .is_some_and(|definition| definition.kind == BehaviorKind::AddExPoint)
        && reward.target_from_condition
        && reward.target.code == 101
        && reward.target.raw.is_empty()
        && predicate.opcode == 19210
        && predicate.type_name == "HasBuffId"
        && matches!(&predicate.kind, ParsedConditionKind::BuffId { mode: BuffConditionMode::Present, buff_ids }
            if matches!(buff_ids.as_slice(), [buff] if *buff > 0))
    {
        reward.delegated_reward_phase = Some(*phase);
    }
}
