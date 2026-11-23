use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    event::payload::BattleEvent,
    manager::ex_point::{ExPointChange, ExPointCommand},
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports_holder(args: &[i32]) -> bool {
    matches!(args, [0, amount] if *amount > 0)
}

pub fn supports_attacker(args: &[i32]) -> bool {
    matches!(args, [1, amount] if *amount < 0)
}

pub fn rule_ops(subscriber: &BuffActSubscriber, event: &BattleEvent) -> Option<Vec<RuleOp>> {
    let BattleEvent::Hit(hit) = event else {
        return None;
    };
    let [target, delta] = subscriber.args.as_slice() else {
        return None;
    };
    if hit.target_uid != subscriber.owner_uid {
        return Some(Vec::new());
    }
    let target_uid = match target {
        0 => subscriber.owner_uid,
        1 => hit.source_uid,
        _ => return None,
    };
    let change = ExPointChange {
        origin: super::command_origin(subscriber)?,
        source_uid: subscriber.owner_uid,
        target_uid,
        delta: *delta,
        config_effect: 0,
        effect_type: EffectType::Expointchange as i32,
    };
    Some(vec![RuleOp::Command(BattleCommand::ExPoint(
        if *delta < 0 {
            ExPointCommand::Reduce(change)
        } else {
            ExPointCommand::Change(change)
        },
    ))])
}

#[cfg(test)]
mod tests;
