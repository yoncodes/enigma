use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    damage::{DamageFormulaInput, calculate, modifiers},
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        hp::{DamageEffectKind, HpCommand, HpDamage, HurtDamageFromType, HurtInfoData},
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
) -> Option<Vec<super::BuffActRuleOp>> {
    let BattleEvent::Hit(hit) = event else {
        return None;
    };
    let [mode, raw_attr, permille] = subscriber.args.as_slice() else {
        return None;
    };
    // "When being attacked": additional damage riding on an attack is not another attack.
    if hit.target_uid != subscriber.owner_uid || hit.damage_from == HurtDamageFromType::Additional {
        return Some(Vec::new());
    }
    let marker = super::BuffActRuleOp::subscriber_from_applier(RuleOp::BuffFeatureMarker {
        target_uid: subscriber.owner_uid,
        effect_type: EffectType::Dot as i32,
        effect_num: subscriber.buff_id,
        buff_act_id: subscriber.key.definition.opcode,
    });
    let attr_uid = match mode {
        0 => subscriber.source_uid,
        1 => subscriber.owner_uid,
        _ => return None,
    };
    let attr_id = AttrId::from_raw(*raw_attr)?;
    if !subscriber.owner_alive {
        return Some(vec![marker]);
    }
    let attr_uid = if attr_uid == 0 {
        subscriber.owner_uid
    } else {
        attr_uid
    };
    let amount = calculate(DamageFormulaInput::genesis(
        managers.origin_attribute(attr_uid, attr_id),
        (*permille).max(0),
        modifiers::genesis_multiplier(managers, subscriber.source_uid, subscriber.owner_uid),
    ));
    if amount <= 0 {
        return Some(vec![marker]);
    }
    Some(vec![
        marker,
        super::BuffActRuleOp::subscriber_from_applier(RuleOp::Command(BattleCommand::Hp(
            HpCommand::Damage(HpDamage {
                origin: super::command_origin(subscriber)?,
                source_uid: subscriber.source_uid,
                target_uid: subscriber.owner_uid,
                amount,
                config_effect: 0,
                effect_kind: DamageEffectKind::Genesis,
                assassinate: false,
                ignore_riposte: false,
                hurt: HurtInfoData {
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
                },
            }),
        ))),
    ])
}

#[cfg(test)]
mod tests;
