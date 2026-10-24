use super::*;
use crate::engine::{
    runtime::skill::SkillExecution,
    skill::action::{SkillExecutionMode, SkillInvocation, SkillRequest, SkillTarget},
};

fn contains_effect(step: &sonettobuf::FightStep, effect_type: i32, target_uid: i64) -> bool {
    step.act_effect.iter().any(|effect| {
        effect.effect_type == Some(effect_type) && effect.target_id == Some(target_uid)
            || effect
                .fight_step
                .as_ref()
                .is_some_and(|child| contains_effect(child, effect_type, target_uid))
    })
}

fn skill_contains_effect(
    step: &sonettobuf::FightStep,
    skill_id: i32,
    effect_type: i32,
    target_uid: i64,
) -> bool {
    step.act_id == Some(skill_id) && contains_effect(step, effect_type, target_uid)
        || step.act_effect.iter().any(|effect| {
            effect.fight_step.as_ref().is_some_and(|child| {
                skill_contains_effect(child, skill_id, effect_type, target_uid)
            })
        })
}

fn find_skill(step: &sonettobuf::FightStep, skill_id: i32) -> Option<&sonettobuf::FightStep> {
    (step.act_id == Some(skill_id)).then_some(step).or_else(|| {
        step.act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
            .find_map(|child| find_skill(child, skill_id))
    })
}

mod death_cleanup;
mod invalidation;
mod targeting_queue;
