use crate::engine::{
    manager::fight_progress::{LegacyProgressAdd, LegacyProgressClear, LegacyProgressInit},
    mechanic::bloodtithe::progress,
    skill::{
        behavior::{
            BehaviorOpContext,
            registry::{BehaviorConfigError, BehaviorHandler},
        },
        condition::{ParsedCondition, ParsedConditionKind, lifecycle::LifecycleMode},
        effect::{ParsedBehavior, SkillEffectCatalog, SkillEffectSlot},
        rule::output::{BattleCommand, RuleOp},
        target::request::SOURCE_TARGET_CODE,
    },
};

pub(super) struct ChangeHandler;

enum Change {
    Add(i32),
    Clear,
}

fn change(behavior: &ParsedBehavior) -> Option<Change> {
    match (behavior.raw_arity, behavior.args.as_slice()) {
        (2, [0, amount]) if *amount > 0 => Some(Change::Add(*amount)),
        (2, [1, 0]) => Some(Change::Clear),
        _ => None,
    }
}

impl BehaviorHandler for ChangeHandler {
    const VALIDATES_ARGUMENTS: bool = true;
    const COMPLETED_ACTION_ACTOR_ONLY: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        change(behavior).is_some()
    }

    fn unsupported_args(behavior: &ParsedBehavior) -> bool {
        behavior.raw_arity == 2 && matches!(behavior.args.as_slice(), [0, i32::MIN..=0])
    }

    fn validate_slot(slot: &SkillEffectSlot, condition: &str) -> Result<(), BehaviorConfigError> {
        let condition = condition.trim();
        if !condition.is_empty()
            && condition
                .split(['#', '&', '|'])
                .any(|part| part.trim().is_empty())
        {
            return Err(BehaviorConfigError::MalformedCondition);
        }
        let source_only = !slot.target_from_condition
            && [&slot.condition_target, &slot.target]
                .into_iter()
                .all(|target| target.code == SOURCE_TARGET_CODE && target.raw.is_empty());
        let supported = match change(&slot.behavior) {
            Some(Change::Add(_)) => {
                matches!(
                    slot.conditions.as_slice(),
                    [condition] if condition.opcode == 8 && condition.kind == ParsedConditionKind::EntityDead
                ) || on_completed_action(&slot.conditions)
                    || progress::matches(slot)
            }
            Some(Change::Clear) => matches!(
                slot.conditions.as_slice(),
                [condition] if condition.raw_args.len() == 1
                    && matches!(&condition.kind, ParsedConditionKind::BuffExpired(ids) if ids.len() == 1)
            ),
            None => false,
        };
        if source_only && supported {
            Ok(())
        } else {
            Err(BehaviorConfigError::Unsupported)
        }
    }

    fn emit_runtime_ops(
        context: BehaviorOpContext<'_>,
        slot: &SkillEffectSlot,
        _: &SkillEffectCatalog,
    ) -> Option<Vec<RuleOp>> {
        if progress::matches(slot) {
            return progress::rule_ops(context, &slot.behavior);
        }
        Self::emit_ops(context, &slot.behavior)
    }

    fn emit_ops(_: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let origin = super::command_origin(behavior)?;
        let command = match change(behavior)? {
            Change::Add(amount) => {
                BattleCommand::LegacyProgressAdd(LegacyProgressAdd { origin, amount })
            }
            Change::Clear => BattleCommand::LegacyProgressClear(LegacyProgressClear { origin }),
        };
        Some(vec![RuleOp::Command(command)])
    }
}

fn on_completed_action(conditions: &[ParsedCondition]) -> bool {
    let [trigger, filters @ ..] = conditions else {
        return false;
    };
    if trigger.opcode != 502212
        || trigger.raw_args.len() != 1
        || trigger.kind != (ParsedConditionKind::ActiveAllyUseSkill { slot: 0 })
    {
        return false;
    }
    match filters {
        [] => true,
        [damage, kind] => {
            damage.opcode == 501212
                && damage.kind == ParsedConditionKind::UseHurtSkill
                && kind.opcode == 500212
                && kind.kind == ParsedConditionKind::DamageTargetCountKind(2)
        }
        _ => false,
    }
}

pub(super) struct InitializeHandler;

impl BehaviorHandler for InitializeHandler {
    const VALIDATES_ARGUMENTS: bool = true;
    const SETUP_ONLY: bool = true;

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

    fn validate_slot(slot: &SkillEffectSlot, raw: &str) -> Result<(), BehaviorConfigError> {
        let opening = matches!(
            (raw.trim(), slot.conditions.as_slice()),
            ("5", [condition])
                if condition.kind == ParsedConditionKind::Lifecycle(LifecycleMode::EnterFight)
        ) || matches!(
            (raw.trim(), slot.conditions.as_slice()),
            ("5021", [condition])
                if condition.kind == ParsedConditionKind::Lifecycle(LifecycleMode::BattleStart)
        );
        let source_only = !slot.target_from_condition
            && [&slot.condition_target, &slot.target]
                .into_iter()
                .all(|target| target.code == SOURCE_TARGET_CODE && target.raw.is_empty());
        if opening && source_only {
            Ok(())
        } else {
            Err(BehaviorConfigError::Unsupported)
        }
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

#[cfg(test)]
mod tests;
