use super::*;
use crate::engine::skill::{action::ActionPayment, rule::output::BattleCommand};

pub(super) fn settle(
    invocation: &SkillInvocation,
    execution: &mut SkillExecution,
    mut completed: SkillEmissionOp,
) -> Vec<SkillEmissionOp> {
    let Some(payment) = execution.take_payment() else {
        return vec![completed];
    };
    execution.record_payment(&payment);
    let ActionPayment {
        cost,
        composition: rewards,
    } = payment;
    if rewards.is_empty() {
        let RuleOp::SkillLifecycle(lifecycle) = completed.op else {
            unreachable!("a completed phase emits a skill lifecycle")
        };
        completed.op = RuleOp::BeginSkillAction { lifecycle, cost };
        return vec![completed];
    }
    let mut ops = std::iter::once(RuleOp::PaySkillAction {
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
    let RuleOp::SkillLifecycle(lifecycle) = completed.op else {
        unreachable!("a completed phase emits a skill lifecycle")
    };
    completed.op = RuleOp::PaidSkillStarted(lifecycle);
    ops.push(completed);
    ops
}
