use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    damage::{DamageFormulaInput, calculate, modifiers},
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{
        BattleManagers, emitter,
        hp::{HpCommand, HpLoss, HurtDamageFromType, HurtInfoData},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [raw_attr, rate, limit]
        if AttrId::from_raw(*raw_attr).is_some() && *rate > 0 && *limit > 0)
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::Hit(hit) = event else {
        return None;
    };
    let [raw_attr, rate, limit] = subscriber.args.as_slice() else {
        return None;
    };
    if !supports(&subscriber.args)
        || !subscriber.owner_alive
        || hit.source_uid != emitter::UID
        || hit.target_uid != subscriber.owner_uid
        || !managers.can_fire_buff_act(
            subscriber.owner_uid,
            subscriber.buff_uid,
            subscriber.key.definition,
            *limit,
        )
    {
        return Some(Vec::new());
    }

    let amount = calculate(DamageFormulaInput::genesis(
        managers.origin_attribute(subscriber.source_uid, AttrId::from_raw(*raw_attr)?),
        *rate,
        modifiers::genesis_multiplier(managers, subscriber.source_uid, subscriber.owner_uid),
    ));
    if amount <= 0 {
        return Some(Vec::new());
    }
    let origin = super::command_origin(subscriber)?;
    Some(vec![
        RuleOp::MarkBuffActFired {
            owner_uid: subscriber.owner_uid,
            buff_uid: subscriber.buff_uid,
            key: subscriber.key.definition,
        },
        RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
            origin,
            source_uid: subscriber.source_uid,
            target_uid: subscriber.owner_uid,
            amount,
            config_effect: 0,
            hurt: Some(HurtInfoData {
                from_uid: subscriber.source_uid,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 0,
                skill_id: 0,
                damage_from: HurtDamageFromType::Buff,
                buff_act_id: subscriber.key.definition.opcode,
                buff_uid: subscriber.buff_uid,
                hurt_effect_type: EffectType::Origindamage as i32,
                display_amount: Some(amount),
            }),
        }))),
    ])
}

#[cfg(test)]
mod tests;
