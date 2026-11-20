use crate::engine::{
    event::kind::EventKind,
    skill::{
        action::SkillPhase,
        behavior::{classify::BehaviorKind, registry},
    },
};

use super::SkillEffectSlot;

pub(super) fn bind_after_damage_grants(slots: &mut [SkillEffectSlot]) {
    let mut follows_after_damage = false;
    for slot in slots.iter_mut() {
        if follows_after_damage && is_predicate_grant(slot) {
            slot.ordered_phase = Some(SkillPhase::AfterHit);
            continue;
        }
        follows_after_damage = slot
            .active_phases()
            .is_ok_and(|phases| phases == [SkillPhase::AfterDamage]);
    }
}

fn is_predicate_grant(slot: &SkillEffectSlot) -> bool {
    !slot.conditions.is_empty()
        && registry::find(&slot.behavior)
            .is_some_and(|definition| definition.kind == BehaviorKind::AddBuff)
        && slot
            .compiled_route
            .as_ref()
            .is_ok_and(|route| route.branches.iter().all(|branch| branch.driver.is_none()))
        && slot.compiled_subscriptions().is_ok_and(|subscriptions| {
            subscriptions
                .iter()
                .all(|subscription| subscription.event == EventKind::SkillAction)
        })
        && slot.active_phases().is_ok_and(|phases| phases.is_empty())
}
