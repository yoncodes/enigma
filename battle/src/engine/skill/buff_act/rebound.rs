use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    damage::{DamageFormulaInput, calculate, modifiers},
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
        hp::{HpCommand, HpLoss, HurtDamageFromType, HurtInfoData},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [0 | 2, permille, raw_attr]
        if *permille > 0 && AttrId::from_raw(*raw_attr).is_some())
}

pub fn supports_damage_based(args: &[i32]) -> bool {
    matches!(args, [permille, 0, 0] if *permille > 0)
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::Hit(hit) = event else {
        return None;
    };
    let [_, permille, raw_attr] = subscriber.args.as_slice() else {
        return None;
    };
    if !supports(&subscriber.args)
        || hit.target_uid != subscriber.owner_uid
        || hit.source_uid == 0
        || managers
            .buff
            .has_buff_act_kind(hit.source_uid, super::registry::BuffActKind::IgnoreRebound)
        || !subscriber.owner_alive
    {
        return Some(Vec::new());
    }
    let amount = calculate(DamageFormulaInput::genesis(
        managers.origin_attribute(subscriber.owner_uid, AttrId::from_raw(*raw_attr)?),
        *permille,
        modifiers::genesis_multiplier(managers, subscriber.owner_uid, hit.source_uid),
    ));
    if amount <= 0 {
        return Some(Vec::new());
    }
    reflection_ops(
        managers,
        subscriber,
        hit.source_uid,
        amount,
        DepletedBuff::Keep,
    )
}

pub fn damage_based_rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::Hit(hit) = event else {
        return None;
    };
    let [permille, 0, 0] = subscriber.args.as_slice() else {
        return None;
    };
    if !supports_damage_based(&subscriber.args)
        || hit.amount <= 0
        || hit.target_uid != subscriber.owner_uid
        || hit.source_uid == 0
        || managers
            .buff
            .has_buff_act_kind(hit.source_uid, super::registry::BuffActKind::IgnoreRebound)
        || !subscriber.owner_alive
    {
        return Some(Vec::new());
    }
    let amount = calculate(DamageFormulaInput::genesis(
        hit.amount,
        *permille,
        modifiers::genesis_multiplier(managers, subscriber.owner_uid, hit.source_uid),
    ))
    .max(1);
    reflection_ops(
        managers,
        subscriber,
        hit.source_uid,
        amount,
        DepletedBuff::Remove,
    )
}

fn reflection_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    target_uid: i64,
    amount: i32,
    depleted: DepletedBuff,
) -> Option<Vec<RuleOp>> {
    let origin = super::command_origin(subscriber)?;
    let mut ops = vec![RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
        HpLoss {
            origin,
            source_uid: subscriber.owner_uid,
            target_uid,
            amount,
            config_effect: 0,
            hurt: Some(HurtInfoData {
                from_uid: subscriber.owner_uid,
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
        },
    )))];
    if managers.catalog().buff_has_effect_count(subscriber.buff_id) {
        let consume = BuffConsume {
            origin,
            target_uid: subscriber.owner_uid,
            selector: BuffSelector::Uid(subscriber.buff_uid),
            amount: 1,
            depleted,
        };
        ops.push(RuleOp::Command(BattleCommand::Buff(
            if matches!(depleted, DepletedBuff::Remove) {
                BuffCommand::ConsumeCount(consume)
            } else {
                BuffCommand::ConsumeEffectCount(consume)
            },
        )));
    }
    Some(ops)
}

#[cfg(test)]
mod tests;
