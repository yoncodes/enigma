use crate::engine::{
    fight::reserve::ClearReserves,
    skill::{
        behavior::{BehaviorOpContext, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        behavior.raw_arity == 0 && behavior.raw_args.is_empty()
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        Some(vec![RuleOp::Command(BattleCommand::ClearReserves(
            ClearReserves {
                origin: super::command_origin(behavior)?,
                owner_uid: context.target_uid,
            },
        ))])
    }
}
