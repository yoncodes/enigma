use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffConsume, BuffGrant, BuffSelector, DepletedBuff},
        emitter,
        eureka::{EUREKA_RESOURCE_ID, EurekaChange, EurekaCommand},
    },
    skill::{
        buff_act::registry::BuffActKind,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [stacks, cost, buff_id, 2] if *stacks > 0 && *cost > 0 && *buff_id > 0)
}

pub fn referenced_buff(args: &[i32]) -> Option<i32> {
    supports(args).then_some(args[2])
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::ConsumeCurBuffAndAuraAddBuff)
        || event.kind() != EventKind::RoundStart
        || !supports(&subscriber.args)
    {
        return None;
    }
    let [stacks, cost, buff_id, 2] = subscriber.args.as_slice() else {
        return None;
    };
    if !subscriber.owner_alive
        || subscriber.amount < *stacks
        || managers
            .eureka
            .get(subscriber.owner_uid, EUREKA_RESOURCE_ID)
            .current
            < *cost
    {
        return Some(Vec::new());
    }

    let origin = super::command_origin(subscriber)?;
    Some(vec![
        RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(EurekaChange {
            origin,
            source_uid: subscriber.owner_uid,
            target_uid: subscriber.owner_uid,
            power_id: EUREKA_RESOURCE_ID,
            delta: -*cost,
            effect_type: EffectType::Powerchange as i32,
        }))),
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: subscriber.owner_uid,
            target_uid: emitter::UID,
            buff_id: *buff_id,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))),
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
            origin,
            target_uid: subscriber.owner_uid,
            selector: BuffSelector::Uid(subscriber.buff_uid),
            amount: *stacks,
            depleted: DepletedBuff::Remove,
        }))),
    ])
}

#[cfg(test)]
mod tests;
