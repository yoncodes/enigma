use crate::engine::{
    manager::entity::position::MoveFrontCommand,
    skill::{
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        matches!(
            (behavior.spec.kind, behavior.args.as_slice()),
            (BehaviorKind::MoveFront, [distance]) if matches!(*distance, 1..=4 | 99)
        )
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        Self::supports(behavior).then(|| {
            vec![RuleOp::Command(BattleCommand::MoveFront(
                MoveFrontCommand {
                    origin: super::command_origin(behavior).expect("registered behavior"),
                    target_uid: context.target_uid,
                    distance: behavior.args[0],
                },
            ))]
        })
    }
}

#[cfg(test)]
mod tests;
