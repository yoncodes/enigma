use crate::engine::{
    fight::reserve::ClearReserves,
    skill::{
        behavior::{
            BehaviorOpContext,
            registry::{BehaviorConfigError, BehaviorHandler},
        },
        condition::ParsedConditionKind,
        effect::{ParsedBehavior, SkillEffectSlot},
        rule::output::{BattleCommand, RuleOp},
        target::request::SOURCE_TARGET_CODE,
    },
};

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        behavior.raw_arity == 0 && behavior.raw_args.is_empty()
    }

    fn validate_slot(slot: &SkillEffectSlot, condition: &str) -> Result<(), BehaviorConfigError> {
        let own_defeat = condition.trim() == "8"
            && matches!(
                slot.conditions.as_slice(),
                [condition] if condition.kind == ParsedConditionKind::EntityDead
            );
        let source_only = !slot.target_from_condition
            && [&slot.condition_target, &slot.target]
                .into_iter()
                .all(|target| target.code == SOURCE_TARGET_CODE && target.raw.is_empty());
        if own_defeat && source_only {
            Ok(())
        } else {
            Err(BehaviorConfigError::Unsupported)
        }
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        Some(vec![RuleOp::Command(BattleCommand::ClearReserves(
            ClearReserves {
                origin: super::command_origin(behavior)?,
                owner_uid: context.source_uid,
            },
        ))])
    }
}
