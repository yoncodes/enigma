use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers, emitter,
        eureka::{EUREKA_RESOURCE_ID, EurekaChange, EurekaCommand},
        gauge::{GaugeCommand, GaugeOperation},
    },
    mechanic::impromptu,
    skill::{
        action::SkillExecutionMode,
        buff_act::registry::BuffActKind,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [cost, inspiration] if *cost > 0 && *inspiration > 0)
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::SkillAction(action) = event else {
        return None;
    };
    if !super::subscriber_is_kind(subscriber, BuffActKind::UseSkillConsumeFromAddEmitterEnergy)
        || action.source_uid != subscriber.owner_uid
        || action.mode != SkillExecutionMode::Active
        || !subscriber.owner_alive
        || !supports(&subscriber.args)
    {
        return Some(Vec::new());
    }
    let [cost, inspiration] = subscriber.args.as_slice() else {
        return None;
    };
    let source_uid = subscriber.source_uid;
    let inspiration_key = impromptu::inspiration_key(emitter::UID);
    if managers.eureka.get(source_uid, EUREKA_RESOURCE_ID).current < *cost
        || managers.gauge.get(inspiration_key).is_none()
    {
        return Some(Vec::new());
    }

    let origin = super::command_origin(subscriber)?;
    Some(vec![
        RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(EurekaChange {
            origin,
            source_uid,
            target_uid: source_uid,
            power_id: EUREKA_RESOURCE_ID,
            delta: -*cost,
            effect_type: EffectType::Powerchange as i32,
        }))),
        RuleOp::Command(BattleCommand::Gauge(GaugeCommand::new(
            origin,
            inspiration_key,
            GaugeOperation::ChangeValue {
                delta: *inspiration,
            },
        ))),
    ])
}

#[cfg(test)]
mod tests;
