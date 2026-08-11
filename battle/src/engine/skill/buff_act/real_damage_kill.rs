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
mod tests {
    use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;
    use crate::engine::{
        event::subscription::SubscriptionKey,
        skill::{rule::DefinitionKey, target::TargetPool},
    };
    use crate::test_support::init_config;

    #[test]
    fn kill_emits_hp_loss_then_its_owned_wire_marker() {
        init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(10),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let subscriber = BuffActSubscriber {
            owner_uid: -1,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 30,
            team_type: 2,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(EVENT, DefinitionKey::new(1028, "RealDamageKill")),
            act_type: "RealDamageKill".to_owned(),
            effect_time: 0,
            effect_condition: 0,
            args: vec![200, AttrId::Attack as i32, 40_000],
            raw: "1028#200#203#40000".to_owned(),
        };

        let ops = rule_ops(&managers, &TargetPool::from_fight(&fight), &subscriber).unwrap();

        assert!(matches!(
            ops.as_slice(),
            [
                RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
                    target_uid: -1,
                    ..
                }))),
                RuleOp::BuffFeatureMarker {
                    target_uid: -1,
                    effect_type,
                    effect_num: 9999,
                    ..
                }
            ] if *effect_type == EffectType::Realdamagekill as i32
        ));
    }

    #[test]
    fn kill_requires_both_the_hp_ratio_and_source_attribute_thresholds() {
        init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(10),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(300),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let subscriber = BuffActSubscriber {
            owner_uid: -1,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 30,
            team_type: 2,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(EVENT, DefinitionKey::new(1028, "RealDamageKill")),
            act_type: "RealDamageKill".to_owned(),
            effect_time: 0,
            effect_condition: 0,
            args: vec![200, AttrId::Attack as i32, 40_000],
            raw: "1028#200#102#40000".to_owned(),
        };

        let managers = BattleManagers::seeded(&fight);
        assert!(
            rule_ops(&managers, &TargetPool::from_fight(&fight), &subscriber)
                .unwrap()
                .is_empty()
        );

        let mut source_limited = fight;
        source_limited.attacker.as_mut().unwrap().entitys[0]
            .attr
            .as_mut()
            .unwrap()
            .attack = Some(1);
        source_limited.defender.as_mut().unwrap().entitys[0].current_hp = Some(150);
        let managers = BattleManagers::seeded(&source_limited);
        assert!(
            rule_ops(
                &managers,
                &TargetPool::from_fight(&source_limited),
                &subscriber,
            )
            .unwrap()
            .is_empty()
        );
    }
}
