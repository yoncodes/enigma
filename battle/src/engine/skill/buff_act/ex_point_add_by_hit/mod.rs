use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
        ex_point::{ExPointChange, ExPointCommand},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, super::registry::BuffActKind::ExPointAddByHit) {
        return None;
    }
    let BattleEvent::Hit(hit) = event else {
        return Some(Vec::new());
    };
    if !subscriber.owner_alive || hit.target_uid != subscriber.owner_uid {
        return Some(Vec::new());
    }
    let layer = managers
        .buff
        .snapshot(subscriber.owner_uid, subscriber.buff_uid)?
        .layer
        .unwrap_or_default();
    if layer <= 0 {
        return Some(Vec::new());
    }
    let origin = super::command_origin(subscriber)?;
    let delta = subscriber.args.first().copied().unwrap_or(1);
    Some(vec![
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
            origin,
            target_uid: subscriber.owner_uid,
            selector: BuffSelector::Uid(subscriber.buff_uid),
            amount: 1,
            depleted: DepletedBuff::Remove,
        }))),
        RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(
            ExPointChange {
                origin,
                source_uid: hit.source_uid,
                target_uid: hit.source_uid,
                delta,
                config_effect: 0,
                effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
            },
        ))),
    ])
}

#[cfg(test)]
mod tests;
