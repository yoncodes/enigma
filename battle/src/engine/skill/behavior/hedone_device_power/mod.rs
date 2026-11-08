use crate::engine::{
    mechanic::hedone_device_power::HedoneDevicePowerCommand,
    skill::{
        behavior::{BehaviorOpContext, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
    },
};

pub struct Handler;

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        matches!(behavior.args.as_slice(), [power_id] if *power_id > 0)
    }

    fn references(_behavior: &ParsedBehavior) -> RuleReferences {
        RuleReferences::default()
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let [power_id] = behavior.args.as_slice() else {
            return None;
        };
        Some(vec![RuleOp::Command(BattleCommand::HedoneDevicePower(
            HedoneDevicePowerCommand {
                origin: super::command_origin(behavior)?,
                owner_uid: context.target_uid,
                power_id: *power_id,
            },
        ))])
    }
}

#[cfg(test)]
mod tests;
