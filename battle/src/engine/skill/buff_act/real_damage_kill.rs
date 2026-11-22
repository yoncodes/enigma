use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    entity::attr::AttrId,
    event::kind::EventKind,
    manager::{
        BattleManagers,
        hp::{HpCommand, HpLoss, HurtDamageFromType, HurtInfoData},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
        target::TargetPool,
    },
};

pub const EVENT: EventKind = EventKind::BeAttacked;

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [_, raw_attr_id, _] if AttrId::from_raw(*raw_attr_id).is_some())
}

pub fn initial_source_attack_threshold(source_attack: i32, args: &[i32]) -> Option<i64> {
    let [_, raw_attr_id, attr_rate_permille] = args else {
        return None;
    };
    (AttrId::from_raw(*raw_attr_id) == Some(AttrId::Attack))
        .then_some(scaled_source_threshold(source_attack, *attr_rate_permille))
}

pub fn rule_ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, super::registry::BuffActKind::RealDamageKill)
        || !supports(&subscriber.args)
    {
        return None;
    }
    let Some(amount) = kill_amount(managers, pool, subscriber) else {
        return Some(Vec::new());
    };
    Some(vec![
        RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
            origin: super::command_origin(subscriber)?,
            source_uid: subscriber.source_uid,
            target_uid: subscriber.owner_uid,
            amount,
            config_effect: 0,
            hurt: Some(hurt(subscriber)),
        }))),
        RuleOp::BuffFeatureMarker {
            target_uid: subscriber.owner_uid,
            effect_type: EffectType::Realdamagekill as i32,
            effect_num: 9999,
            buff_act_id: 0,
        },
    ])
}

fn kill_amount(
    managers: &BattleManagers,
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
) -> Option<i32> {
    let [threshold_permille, raw_attr_id, attr_rate_permille] = subscriber.args.as_slice() else {
        return None;
    };
    let target = managers.hp.get(subscriber.owner_uid);
    if target.current <= 0 {
        return None;
    }
    let attr_id = AttrId::from_raw(*raw_attr_id)?;
    let source = pool.entity(subscriber.source_uid)?;
    let source_attr = match attr_id {
        AttrId::Attack => {
            let rate = 1000
                + managers.attribute.get(source.uid, AttrId::Attack)
                + managers.buff.attribute_delta(source.uid, AttrId::Attack);
            managers.attribute.base(source.uid, AttrId::Attack) * rate / 1000
        }
        _ => managers.attribute.get(source.uid, attr_id),
    };
    let source_threshold = scaled_source_threshold(source_attr, *attr_rate_permille);
    let below_hp_threshold = target.max > 0
        && (target.current as i64 * 1000) <= target.max as i64 * *threshold_permille as i64;
    (below_hp_threshold && i64::from(target.current) <= source_threshold).then_some(target.current)
}

fn scaled_source_threshold(source_attr: i32, attr_rate_permille: i32) -> i64 {
    i64::from(source_attr) * i64::from(attr_rate_permille) / 1000
}

fn hurt(subscriber: &BuffActSubscriber) -> HurtInfoData {
    HurtInfoData {
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
        display_amount: Some(9999),
    }
}

#[cfg(test)]
mod tests;
