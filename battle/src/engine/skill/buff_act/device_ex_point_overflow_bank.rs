use sonettobuf::{BuffActInfo, effect_type_enum::EffectType};

use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{BuffAccumulateCappedActState, BuffActInfoMarkerResult, BuffCommand, BuffSetState},
        ex_point::{ExPointChange, ExPointCommand, ExPointKind},
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
        super::registry::BuffActKind::DeviceExPointOverflowBank,
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
        subscriber.key.definition.opcode,
    )?;
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
            effect_type: EffectType::Expointchange as i32,
        })),
    ))])
}

fn store_rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    change: &crate::engine::event::payload::ExPointChangeEvent,
) -> Option<Vec<BuffActRuleOp>> {
    let [capacity] = subscriber.args.as_slice() else {
        return None;
    };
    if change.overflow <= 0
        || change.kind != ExPointKind::DevicePower
        || change.target_uid != subscriber.owner_uid
        || change.origin == super::command_origin(subscriber)?
    {
        return Some(Vec::new());
    }
    let current = current(
        &managers
            .buff
            .snapshot(subscriber.owner_uid, subscriber.buff_uid)?,
        subscriber.key.definition.opcode,
    )?;
    let maximum = capacity.saturating_mul(subscriber.amount.max(1));
    let stored = change.overflow.min((maximum - current).max(0));
    if stored <= 0 {
        return Some(Vec::new());
    }
    Some(vec![BuffActRuleOp::grouped_event(RuleOp::Command(
        BattleCommand::Buff(BuffCommand::AccumulateCappedActState(
            BuffAccumulateCappedActState {
                origin: super::command_origin(subscriber)?,
                target_uid: subscriber.owner_uid,
                buff_uid: subscriber.buff_uid,
                act_id: subscriber.key.definition.opcode,
                delta: stored,
                maximum,
            },
        )),
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
        || change.kind != ExPointKind::DevicePower
        || change.applied_delta <= 0
    {
        return Some(Vec::new());
    }
    let buff = managers
        .buff
        .snapshot(subscriber.owner_uid, subscriber.buff_uid)?;
    let current = current(&buff, subscriber.key.definition.opcode)?;
    let after = current - change.applied_delta.min(current);
    if after == current {
        return Some(Vec::new());
    }
    let mut act_info = buff.act_info;
    set_current(&mut act_info, subscriber.key.definition.opcode, after)?;
    let origin = super::command_origin(subscriber)?;
    Some(vec![
        BuffActRuleOp::causing(RuleOp::Command(BattleCommand::Buff(
            BuffCommand::SetInternalState(BuffSetState {
                origin,
                target_uid: subscriber.owner_uid,
                buff_uid: subscriber.buff_uid,
                ex_info: None,
                params: None,
                act_info: Some(act_info),
            }),
        ))),
        BuffActRuleOp::causing(RuleOp::BuffActInfoMarker(BuffActInfoMarkerResult {
            target_uid: subscriber.owner_uid,
            buff_uid: subscriber.buff_uid,
            act_id: subscriber.key.definition.opcode,
            params: vec![after],
            str_param: Some(String::new()),
            team_type: 0,
        })),
    ])
}

fn current(buff: &sonettobuf::BuffInfo, act_id: i32) -> Option<i32> {
    let mut matching = buff
        .act_info
        .iter()
        .filter(|info| info.act_id == Some(act_id));
    let info = matching.next()?;
    if matching.next().is_some() || info.str_param.as_deref() != Some("") {
        return None;
    }
    let [value] = info.param.as_slice() else {
        return None;
    };
    (*value >= 0).then_some(*value)
}

fn set_current(act_info: &mut [BuffActInfo], act_id: i32, value: i32) -> Option<()> {
    let mut matching = act_info
        .iter_mut()
        .filter(|info| info.act_id == Some(act_id));
    let info = matching.next()?;
    if matching.next().is_some() || info.str_param.as_deref() != Some("") {
        return None;
    }
    info.param = vec![value];
    info.str_param = Some(String::new());
    Some(())
}

#[cfg(test)]
mod tests;
