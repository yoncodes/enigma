use crate::engine::{
    event::payload::BattleEvent,
    manager::{BattleManagers, buff::{BuffCommand, BuffConsume, BuffGrantChild, BuffSelector, DepletedBuff},
        eureka::{EurekaChange, EurekaCommand, EUREKA_RESOURCE_ID}},
    skill::{rule::output::{BattleCommand, RuleOp}, subscriber::BuffActSubscriber},
};
use sonettobuf::effect_type_enum::EffectType;

pub fn rule_ops(managers: &BattleManagers, subscriber: &BuffActSubscriber, event: &BattleEvent) -> Option<Vec<RuleOp>> {
    if !matches!(event, BattleEvent::RoundStart) || subscriber.buff_id != 31410202
        || subscriber.args.as_slice() != [1, 2, 31410203, 2] {
        return None;
    }
    let owner = subscriber.owner_uid;
    let emitter = crate::engine::manager::emitter::UID;
    if !subscriber.owner_alive || subscriber.amount < 1
        || managers.eureka.get(owner, EUREKA_RESOURCE_ID).current < 2
        || managers.hp.current(emitter) <= 0 {
        return Some(Vec::new());
    }
    let origin = super::command_origin(subscriber)?;
    Some(vec![
        RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(EurekaChange {
            origin, source_uid: owner, target_uid: owner, power_id: EUREKA_RESOURCE_ID,
            delta: -2, effect_type: EffectType::Powerchange as i32,
        }))),
        RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantChild(BuffGrantChild {
            origin, source_uid: owner, target_uid: emitter, buff_id: 31410203,
            amount: None, params: None, act_info: None,
        }))),
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
            origin, target_uid: owner, selector: BuffSelector::Uid(subscriber.buff_uid),
            amount: 1, depleted: DepletedBuff::Remove,
        }))),
    ])
}
