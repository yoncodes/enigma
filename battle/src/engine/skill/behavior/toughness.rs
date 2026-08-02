use crate::engine::{
    manager::toughness::ToughnessRecover,
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
        behavior.args.is_empty()
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let origin = super::command_origin(behavior)?;
        Self::supports(behavior).then(|| {
            vec![RuleOp::Command(BattleCommand::ToughnessRecover(
                ToughnessRecover {
                    origin,
                    target_uid: context.target_uid,
                    config_effect: behavior.config_effect,
                },
            ))]
        })
    }
}
