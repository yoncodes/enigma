use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{BuffAccumulateActValue, BuffCommand, BuffGrant},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

use super::registry::BuffActKind;

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::GuardBreakReward) {
        return None;
    }
    let [buff_id, per_break] = subscriber.args.as_slice() else {
        return None;
    };
    let origin = super::command_origin(subscriber)?;
    let counter = |delta| {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::AccumulateActValue(
            BuffAccumulateActValue {
                origin,
                target_uid: subscriber.owner_uid,
                buff_uid: subscriber.buff_uid,
                act_id: subscriber.key.definition.opcode,
                delta,
            },
        )))
    };
    if !subscriber.owner_alive {
        return Some(Vec::new());
    }
    if let BattleEvent::ToughnessBroken { target_uid, .. } = event {
        let owner_team = managers.entity.team_type(subscriber.owner_uid)?;
        let target_team = managers.entity.team_type(*target_uid)?;
        return Some(if owner_team != target_team {
            vec![counter(1)]
        } else {
            Vec::new()
        });
    }
    if !matches!(event, BattleEvent::Kind(EventKind::SmallRoundEnd)) {
        return None;
    }
    let count = managers
        .buff
        .act_value(subscriber.buff_uid, subscriber.key.definition.opcode);
    if count < 0 {
        return None;
    }
    if count == 0 {
        return Some(Vec::new());
    }
    Some(vec![
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: subscriber.owner_uid,
            target_uid: subscriber.owner_uid,
            buff_id: *buff_id,
            amount: Some(count.checked_mul(*per_break)?),
            occurrences: 1,
            child_uid_reservations: 0,
        }))),
        counter(-count),
    ])
}

#[cfg(test)]
mod tests;
