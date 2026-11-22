use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffAccumulateActValue, BuffCommand, BuffGrant},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [buff_id, divisor] if *buff_id > 0 && *divisor > 0)
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, super::registry::BuffActKind::LostHpCountAddBuff)
        || !supports(&subscriber.args)
    {
        return None;
    }
    let [buff_id, divisor] = subscriber.args.as_slice() else {
        return None;
    };
    let origin = super::command_origin(subscriber)?;
    if matches!(
        event,
        BattleEvent::HpLost { target_uid, .. } if *target_uid == subscriber.owner_uid
    ) {
        return Some(vec![RuleOp::Command(BattleCommand::Buff(
            BuffCommand::AccumulateActValue(BuffAccumulateActValue {
                origin,
                target_uid: subscriber.owner_uid,
                buff_uid: subscriber.buff_uid,
                act_id: subscriber.key.definition.opcode,
                delta: 1,
            }),
        ))]);
    }
    if !matches!(event, BattleEvent::BuffsSettled(_)) {
        return Some(Vec::new());
    }

    let count = managers
        .buff
        .act_value(subscriber.buff_uid, subscriber.key.definition.opcode);
    let amount = count / divisor;
    let mut ops = Vec::with_capacity(2);
    if amount > 0 {
        ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
            BuffGrant {
                origin,
                source_uid: subscriber.owner_uid,
                target_uid: subscriber.owner_uid,
                buff_id: *buff_id,
                amount: Some(amount),
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))));
    }
    if count > 0 {
        ops.push(RuleOp::Command(BattleCommand::Buff(
            BuffCommand::AccumulateActValue(BuffAccumulateActValue {
                origin,
                target_uid: subscriber.owner_uid,
                buff_uid: subscriber.buff_uid,
                act_id: subscriber.key.definition.opcode,
                delta: -count,
            }),
        )));
    }
    Some(ops)
}

#[cfg(test)]
mod tests;
