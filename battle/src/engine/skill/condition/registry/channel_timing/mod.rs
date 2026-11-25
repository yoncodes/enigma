use crate::engine::{
    entity::passive::PassiveSourceKind,
    event::subscription::ReactionTiming,
    skill::{
        behavior::classify::BehaviorKind,
        condition::ParsedConditionKind,
        effect::{SkillEffectSlot, catalog::SkillEffectTag},
        target::request::SOURCE_TARGET_CODE,
    },
};

pub(super) fn resolve(source: PassiveSourceKind, slot: Option<&SkillEffectSlot>) -> ReactionTiming {
    let Some(slot) = slot.filter(|slot| {
        source == PassiveSourceKind::Psychube
            && slot.behavior.spec.kind == BehaviorKind::AddBuff
            && slot.condition_target.code == SOURCE_TARGET_CODE
            && slot.target.code == SOURCE_TARGET_CODE
            && !slot.target_from_condition
    }) else {
        return ReactionTiming::AfterSkill;
    };
    let mut tags = slot.conditions.iter().filter_map(|condition| {
        let ParsedConditionKind::ActiveSkillEffectTag(tags) = &condition.kind else {
            return None;
        };
        Some(tags.as_slice())
    });
    if tags.next() == Some(&[SkillEffectTag::Channel as i32]) && tags.next().is_none() {
        ReactionTiming::Immediate
    } else {
        ReactionTiming::AfterSkill
    }
}

#[cfg(test)]
mod tests;
