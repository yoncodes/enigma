use crate::engine::skill::{
    behavior::{classify::BehaviorKind, registry},
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

/// A highest-Moxie card that casts a delegated Ultimate grants its later Moxie
/// in the cast's phase, after the Ultimate and its refund.
pub(super) fn bind_reward(slots: &mut [SkillEffectSlot], logic_target: &TargetRequest) {
    if !TargetResolver::is_highest_moxie(logic_target) {
        return;
    }
    let Some(index) = slots.iter().position(is_cast_slot) else {
        return;
    };
    let (head, rewards) = slots.split_at_mut(index + 1);
    let Ok(phases) = head[index].active_phases() else {
        return;
    };
    let [phase] = phases.as_slice() else {
        return;
    };
    for reward in rewards.iter_mut().filter(|slot| {
        registry::find(&slot.behavior)
            .is_some_and(|definition| definition.kind == BehaviorKind::AddExPoint)
    }) {
        reward.delegated_reward_phase = Some(*phase);
    }
}
