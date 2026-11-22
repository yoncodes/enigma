use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        buff::{ActiveBuffFeature, BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
        hp::HpManager,
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

use super::{is_kind, registry::BuffActKind};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [permille] if *permille > 0)
}

pub fn cap(feature: &ActiveBuffFeature, hp: &HpManager) -> Option<i32> {
    if !is_kind(feature, BuffActKind::DamageNotMoreThan) {
        return None;
    }
    let [_, permille] = feature.values.as_slice() else {
        return None;
    };
    Some(
        (i64::from(hp.max(feature.owner_uid)) * i64::from(*permille) / 1000)
            .clamp(0, i64::from(i32::MAX)) as i32,
    )
}

pub fn consume_after_hit(
    managers: &crate::engine::manager::BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::DamageNotMoreThan) {
        return None;
    }
    let BattleEvent::Hit(hit) = event else {
        return Some(Vec::new());
    };
    if hit.target_uid != subscriber.owner_uid
        || !managers.catalog().buff_has_effect_count(subscriber.buff_id)
    {
        return Some(Vec::new());
    }
    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::Consume(BuffConsume {
            origin: super::command_origin(subscriber)?,
            target_uid: subscriber.owner_uid,
            selector: BuffSelector::Uid(subscriber.buff_uid),
            amount: 1,
            depleted: DepletedBuff::Remove,
        }),
    ))])
}

#[cfg(test)]
mod tests;
