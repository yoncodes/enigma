use crate::engine::{
    manager::fight_progress::{LegacyProgressAdd, LegacyProgressInit},
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

pub(super) struct AddHandler;

impl BehaviorHandler for AddHandler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        behavior.raw_arity == 2 && matches!(behavior.args.as_slice(), [0, amount] if *amount > 0)
    }

    fn unsupported_args(behavior: &ParsedBehavior) -> bool {
        behavior.raw_arity == 2 && matches!(behavior.args.as_slice(), [1, 0] | [0, i32::MIN..=0])
    }

    fn validate_slot(slot: &SkillEffectSlot) -> Result<(), BehaviorConfigError> {
        let applies_on_death = matches!(
            slot.conditions.as_slice(),
            [condition] if condition.opcode == 8 && condition.kind == ParsedConditionKind::EntityDead
        );
        let source_only = !slot.target_from_condition
            && [&slot.condition_target, &slot.target]
                .into_iter()
                .all(|target| target.code == SOURCE_TARGET_CODE && target.raw.is_empty());
        if applies_on_death && source_only {
            Ok(())
        } else {
            Err(BehaviorConfigError::Unsupported)
        }
    }

    fn emit_ops(_: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let [0, amount] = *behavior.args.as_slice() else {
            return None;
        };
        if amount <= 0 {
            return None;
        }
        Some(vec![RuleOp::Command(BattleCommand::LegacyProgressAdd(
            LegacyProgressAdd {
                origin: super::command_origin(behavior)?,
                amount,
            },
        ))])
    }
}

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
