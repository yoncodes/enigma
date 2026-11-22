use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::buff::{BuffCommand, BuffGrant},
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

use super::registry::BuffActKind;

pub fn rule_ops(subscriber: &BuffActSubscriber, event: &BattleEvent) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::AddToBuffEntity)
        || event.kind() != EventKind::RoundEndAfterSettlement
    {
        return None;
    }
    let [buff_id, layer] = subscriber.args.as_slice() else {
        return None;
    };
    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::Grant(BuffGrant {
            origin: super::command_origin(subscriber)?,
            source_uid: if subscriber.source_uid != 0 {
                subscriber.source_uid
            } else {
                subscriber.owner_uid
            },
            target_uid: subscriber.owner_uid,
            buff_id: *buff_id,
            amount: Some(*layer),
            occurrences: 1,
            child_uid_reservations: 0,
        }),
    ))])
}

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [buff_id, layer] if *buff_id > 0 && *layer > 0)
}

#[cfg(test)]
mod tests;
