use crate::engine::{
    manager::fight_progress::LegacyProgressInit,
    skill::{
        behavior::{BehaviorOpContext, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};

pub(super) struct InitializeHandler;

impl BehaviorHandler for InitializeHandler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        behavior.raw_arity == 3
            && matches!(
                behavior.args.as_slice(),
                [initial, max, show_id] if *max > 0
                    && (0..=*max).contains(initial)
                    && *show_id > 0
            )
    }

    fn unsupported_args(behavior: &ParsedBehavior) -> bool {
        behavior.raw_arity == 2
            && matches!(
                behavior.args.as_slice(),
                [initial, max] if *max > 0 && (0..=*max).contains(initial)
            )
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let [initial, max, show_id] = *behavior.args.as_slice() else {
            return None;
        };
        Some(vec![RuleOp::Command(BattleCommand::LegacyProgressInit(
            LegacyProgressInit {
                origin: super::command_origin(behavior)?,
                initial,
                max,
                show_id,
                skill_id: context.active_skill_id,
            },
        ))])
    }
}
