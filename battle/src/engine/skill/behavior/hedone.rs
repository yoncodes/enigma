use crate::engine::{
    manager::hedone_counter::{HedoneCounterClearCommand, HedoneDamageCommand},
    skill::{
        behavior::{BehaviorOpContext, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};

pub(super) struct Handler;
pub(super) struct ClearHandler;

impl BehaviorHandler for ClearHandler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        behavior.args.is_empty()
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        Some(vec![RuleOp::Command(BattleCommand::HedoneCounterClear(
            HedoneCounterClearCommand {
                origin: super::command_origin(behavior)?,
                source_uid: context.source_uid,
            },
        ))])
    }
}

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        matches!(behavior.args.as_slice(), [rate] if *rate > 0)
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let [rate] = behavior.args.as_slice() else {
            return None;
        };
        Some(vec![RuleOp::Command(BattleCommand::HedoneDamage(
            HedoneDamageCommand {
                origin: super::command_origin(behavior)?,
                source_uid: context.source_uid,
                target_uid: context.target_uid,
                rate: *rate,
                config_effect: behavior.config_effect,
                skill_id: context.active_skill_id,
            },
        ))])
    }
}

#[cfg(test)]
mod tests;
