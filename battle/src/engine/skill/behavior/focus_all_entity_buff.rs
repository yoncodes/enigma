use crate::engine::{
    mechanic::focus_all_entity_buff::FocusAllEntityBuffCommand,
    skill::{
        behavior::{BehaviorOpContext, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};

pub(super) struct Handler;

pub fn supports_arguments(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [buff_id] if *buff_id > 0)
}

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let buff_id = behavior.arg(0)?;
        if buff_id <= 0 {
            return None;
        }
        let capacity = context.managers.buff.stack_limit(buff_id);
        let current = context
            .managers
            .buff
            .buff_id_amount(context.target_uid, buff_id);
        if current <= 0 || current >= capacity {
            return Some(Vec::new());
        }
        let candidate_uids = context
            .pool
            .active_entities()
            .map(|entity| entity.uid)
            .filter(|uid| {
                *uid != context.source_uid
                    && context.managers.buff.has_active_buff_id(*uid, buff_id)
            })
            .take((capacity - current) as usize)
            .collect::<Vec<_>>();
        if candidate_uids.is_empty() {
            return Some(Vec::new());
        }
        Some(vec![RuleOp::Command(BattleCommand::FocusAllEntityBuff(
            FocusAllEntityBuffCommand {
                origin: super::command_origin(behavior)?,
                source_uid: context.source_uid,
                target_uid: context.target_uid,
                buff_id,
                candidate_uids,
            },
        ))])
    }
}

#[cfg(test)]
mod tests;
