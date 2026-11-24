use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        buff::{BuffCommand, BuffGrant, configured_timed_stacks},
        ex_point::ExPointKind,
    },
    skill::{
        behavior::{
            BehaviorOpContext,
            registry::{BehaviorConfigError, BehaviorHandler},
        },
        effect::{ParsedBehavior, SkillEffectSlot},
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
        target::request::SOURCE_TARGET_CODE,
    },
};

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        behavior.raw_arity == 1 && matches!(behavior.args.as_slice(), [buff_id] if *buff_id > 0)
    }

    fn validate_config(
        game: &config::GameDB,
        behavior: &ParsedBehavior,
    ) -> Result<(), BehaviorConfigError> {
        match behavior
            .arg(0)
            .and_then(|buff_id| configured_timed_stacks(game, buff_id))
        {
            Some(true) => Ok(()),
            Some(false) => Err(BehaviorConfigError::Unsupported),
            None => Err(BehaviorConfigError::MissingBuff),
        }
    }

    fn supports_slot(slot: &SkillEffectSlot) -> bool {
        slot.condition_target.code == 101
            && slot.condition_target.raw.is_empty()
            && slot.target.code == SOURCE_TARGET_CODE
            && slot.target.raw.is_empty()
            && !slot.target_from_condition
            && matches!(slot.conditions.as_slice(), [condition] if
                condition.opcode == 52008 && condition.type_name == "None"
                    && condition.raw_args.is_empty())
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        RuleReferences {
            buffs: behavior.arg(0).into_iter().collect(),
            ..RuleReferences::default()
        }
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let [buff_id] = behavior.args.as_slice() else {
            return None;
        };
        let BattleEvent::ExPointChanged(change) = context.event? else {
            return Some(Vec::new());
        };
        if !change.spent
            || change.kind != ExPointKind::Common
            || change.applied_delta >= 0
            || !context
                .pool
                .main_allies(context.source_uid)
                .iter()
                .any(|ally| ally.uid == change.target_uid)
        {
            return Some(Vec::new());
        }

        Some(vec![RuleOp::Command(BattleCommand::Buff(
            BuffCommand::Grant(BuffGrant {
                origin: super::command_origin(behavior)?,
                source_uid: context.source_uid,
                target_uid: context.target_uid,
                buff_id: *buff_id,
                amount: Some(-change.applied_delta),
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        ))])
    }
}
