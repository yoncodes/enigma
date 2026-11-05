use super::*;
use crate::engine::skill::rule::output::BattleCommand;

pub(super) fn settle(
    invocation: &SkillInvocation,
    execution: &mut SkillExecution,
    mut completed: SkillEmissionOp,
) -> Vec<SkillEmissionOp> {
    let cost = execution.take_action_cost();
    let rewards = execution.take_composition_rewards();
    if rewards.is_empty() {
        if let Some(cost) = cost {
            let RuleOp::SkillLifecycle(lifecycle) = completed.op else {
                unreachable!("a completed phase emits a skill lifecycle")
            };
            completed.op = RuleOp::BeginSkillAction { lifecycle, cost };
        }
        return vec![completed];
    }
    let paid = cost.is_some();
    let mut ops = cost
        .into_iter()
        .map(|cost| RuleOp::PaySkillAction {
            action: invocation.plan,
            cost,
        })
        .chain(rewards.into_iter().map(|change| {
            RuleOp::Command(BattleCommand::ExPoint(
                crate::engine::manager::ex_point::ExPointCommand::Change(change),
            ))
        }))
        .map(|op| SkillEmissionOp {
            op,
            owner: completed.owner,
            consequence: completed.consequence,
            frame_owner: completed.frame_owner.clone(),
        })
        .collect::<Vec<_>>();
    if paid {
        let RuleOp::SkillLifecycle(lifecycle) = completed.op else {
            unreachable!("a completed phase emits a skill lifecycle")
        };
        completed.op = RuleOp::PaidSkillStarted(lifecycle);
    }
    ops.push(completed);
    ops
}
