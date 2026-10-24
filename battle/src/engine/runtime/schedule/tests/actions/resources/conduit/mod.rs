use super::*;

mod completion;
mod device_ultimate;
mod everecho;

fn find_step(step: &sonettobuf::FightStep, act_id: i32) -> Option<&sonettobuf::FightStep> {
    (step.act_id == Some(act_id)).then_some(step).or_else(|| {
        step.act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
            .find_map(|child| find_step(child, act_id))
    })
}
mod action_setup;
mod loop_lifecycle;
mod team_pass;
