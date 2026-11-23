use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::ex_point::{ExPointChange, ExPointCommand},
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [1])
}

pub fn rule_ops(subscriber: &BuffActSubscriber, event: &BattleEvent) -> Option<Vec<RuleOp>> {
    let [amount] = subscriber.args.as_slice() else {
        return None;
    };
    if !supports(&subscriber.args) {
        return None;
    }
    if !matches!(event, BattleEvent::Kind(EventKind::RoundStartCard)) {
        return Some(Vec::new());
    }
    Some(vec![RuleOp::Command(BattleCommand::ExPoint(
        ExPointCommand::Change(ExPointChange {
            origin: super::command_origin(subscriber)?,
            source_uid: subscriber.owner_uid,
            target_uid: subscriber.owner_uid,
            delta: *amount,
            config_effect: 0,
            effect_type: EffectType::Expointchange as i32,
        }),
    ))])
}

#[cfg(test)]
mod tests;
