use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffSetState},
        ex_point::{ExPointChange, ExPointCommand},
    },
    skill::{
        buff_act::BuffActRuleOp,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<BuffActRuleOp>> {
    if !super::subscriber_is_kind(
        subscriber,
        super::registry::BuffActKind::ExPointOverflowBank,
    ) {
        return Some(Vec::new());
    }
    match event {
        BattleEvent::Kind(EventKind::PostActionRefill) => {
            replenishment_rule_ops(managers, subscriber)
        }
        BattleEvent::ExPointOverflow(change) => store_rule_ops(managers, subscriber, change),
        BattleEvent::ExPointChanged(change) => consume_rule_ops(managers, subscriber, change),
        _ => Some(Vec::new()),
    }
}

fn replenishment_rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
) -> Option<Vec<BuffActRuleOp>> {
    let stored = current(
        &managers
            .buff
            .snapshot(subscriber.owner_uid, subscriber.buff_uid)?,
    );
    if stored <= 0
        || !subscriber.owner_alive
        || managers.ex_point.is_full(subscriber.owner_uid)
        || managers.buff.has_buff_act_kind(
            subscriber.owner_uid,
            super::registry::BuffActKind::ExPointCantAdd,
        )
        || managers.buff.has_buff_act_kind(
            subscriber.owner_uid,
            super::registry::BuffActKind::TransferAddExPoint,
        )
    {
        return Some(Vec::new());
    }
    let origin = super::command_origin(subscriber)?;
    Some(vec![BuffActRuleOp::subscriber(RuleOp::Command(
        BattleCommand::ExPoint(ExPointCommand::Change(ExPointChange {
            origin,
            source_uid: subscriber.owner_uid,
            target_uid: subscriber.owner_uid,
            delta: stored,
            config_effect: 0,
            effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
        })),
    ))])
}

fn store_rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    change: &crate::engine::event::payload::ExPointChangeEvent,
) -> Option<Vec<BuffActRuleOp>> {
    if change.overflow <= 0
        || change.origin == super::command_origin(subscriber)?
        || !change
            .kind
            .can_bank_overflow(change.kind, change.target_uid, subscriber.owner_uid)
    {
        return Some(Vec::new());
    }
    let capacity = subscriber.args.first().copied()? * subscriber.amount.max(1);
    let buff = managers
        .buff
        .snapshot(subscriber.owner_uid, subscriber.buff_uid)?;
    let current = current(&buff);
    let stored = stored_overflow(change.overflow, capacity, current);
    if stored <= 0 {
        return Some(Vec::new());
    }
    Some(vec![BuffActRuleOp::grouped_event(RuleOp::Command(
        BattleCommand::Buff(BuffCommand::SetStateSnapshot(BuffSetState {
            ex_info: None,
            origin: super::command_origin(subscriber)?,
            target_uid: subscriber.owner_uid,
            buff_uid: subscriber.buff_uid,
            params: Some(format!(
                "{}#{}",
                subscriber.key.definition.opcode,
                current + stored
            )),
            act_info: None,
        })),
    ))])
}

fn consume_rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    change: &crate::engine::event::payload::ExPointChangeEvent,
) -> Option<Vec<BuffActRuleOp>> {
    if change.origin != super::command_origin(subscriber)?
        || change.source_uid != subscriber.owner_uid
        || change.target_uid != subscriber.owner_uid
        || change.applied_delta <= 0
    {
        return Some(Vec::new());
    }
    let buff = managers
        .buff
        .snapshot(subscriber.owner_uid, subscriber.buff_uid)?;
    let current = current(&buff);
    let restored = change.applied_delta.min(current);
    if restored <= 0 {
        return Some(Vec::new());
    }
    Some(vec![BuffActRuleOp::causing(RuleOp::Command(
        BattleCommand::Buff(BuffCommand::SetStateSnapshot(BuffSetState {
            ex_info: None,
            origin: super::command_origin(subscriber)?,
            target_uid: subscriber.owner_uid,
            buff_uid: subscriber.buff_uid,
            params: Some(format!(
                "{}#{}",
                subscriber.key.definition.opcode,
                current - restored
            )),
            act_info: None,
        })),
    ))])
}

fn current(buff: &sonettobuf::BuffInfo) -> i32 {
    buff.act_common_params
        .as_deref()
        .and_then(|raw| raw.rsplit('#').next())
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or_default()
}

fn stored_overflow(overflow: i32, capacity: i32, current: i32) -> i32 {
    overflow.min((capacity - current).max(0))
}

#[cfg(test)]
mod tests;
