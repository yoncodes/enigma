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
    reflection_ops(subscriber, hit.source_uid, amount, DepletedBuff::Keep)
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
    reflection_ops(subscriber, hit.source_uid, amount, DepletedBuff::Remove)
}

fn reflection_ops(
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
    if config::try_get()
        .and_then(|db| db.skill_buff.get(subscriber.buff_id))
        .is_some_and(|buff| buff.effect_count > 0)
    {
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
mod tests {
    use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;
    use crate::engine::{
        event::payload::HitEvent,
        skill::{
            rule::{CommandOrigin, DefinitionKey, RuleDomain},
            subscriber::BuffActSubscriber,
        },
    };

    #[test]
    fn rebound_uses_holder_attack_and_consumes_trigger_count() {
        crate::test_support::init_config();
        let fight = Fight {
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(2_000),
                    attr: Some(HeroAttribute {
                        attack: Some(1_237),
                        hp: Some(2_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(2_000),
                    attr: Some(HeroAttribute {
                        hp: Some(2_000),
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
            source_uid: -1,
            buff_uid: 1,
            buff_id: 530000411,
            team_type: 2,
            owner_alive: true,
            amount: 1,
            key: crate::engine::event::subscription::SubscriptionKey::new(
                crate::engine::event::kind::EventKind::BeAttacked,
                DefinitionKey::new(303, "Rebound"),
            ),
            act_type: "Rebound".to_owned(),
            effect_time: 209,
            effect_condition: 0,
            args: vec![2, 1_000, AttrId::Attack as i32],
            raw: "303#2#1000#102".to_owned(),
        };
        let event = BattleEvent::Hit(HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "Damage"),
            },
            source_uid: 10,
            target_uid: -1,
            skill_id: 1,
            amount: 100,
            shield_absorbed: 0,
            damage_from: HurtDamageFromType::Skill,
            assassinate: false,
            ignore_riposte: false,
        });
        let ops = rule_ops(&managers, &subscriber, &event).unwrap();

        assert!(matches!(
            ops.as_slice(),
            [
                RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
                    target_uid: 10,
                    amount: 1_237,
                    ..
                }))),
                RuleOp::Command(BattleCommand::Buff(BuffCommand::ConsumeEffectCount(
                    BuffConsume {
                        amount: 1,
                        depleted: DepletedBuff::Keep,
                        ..
                    }
                )))
            ]
        ));
        assert!(!supports(&[1, 100, AttrId::Hp as i32]));
    }

    #[test]
    fn ignored_rebound_emits_no_reflected_damage() {
        crate::test_support::init_config();
        let fight = Fight {
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(2_000),
                    attr: Some(HeroAttribute {
                        attack: Some(1_237),
                        hp: Some(2_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(2_000),
                    buffs: vec![sonettobuf::BuffInfo {
                        uid: Some(20),
                        buff_id: Some(30860141),
                        from_uid: Some(10),
                        count: Some(1),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let subscriber = BuffActSubscriber {
            owner_uid: -1,
            source_uid: -1,
            buff_uid: 1,
            buff_id: 530000411,
            team_type: 2,
            owner_alive: true,
            amount: 1,
            key: crate::engine::event::subscription::SubscriptionKey::new(
                crate::engine::event::kind::EventKind::BeAttacked,
                DefinitionKey::new(303, "Rebound"),
            ),
            act_type: "Rebound".to_owned(),
            effect_time: 209,
            effect_condition: 0,
            args: vec![2, 1_000, AttrId::Attack as i32],
            raw: "303#2#1000#102".to_owned(),
        };
        let event = BattleEvent::Hit(HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "Damage"),
            },
            source_uid: 10,
            target_uid: -1,
            skill_id: 1,
            amount: 100,
            shield_absorbed: 0,
            damage_from: HurtDamageFromType::Skill,
            assassinate: false,
            ignore_riposte: false,
        });

        assert!(rule_ops(&managers, &subscriber, &event).unwrap().is_empty());
    }

    #[test]
    fn damage_based_rebound_uses_damage_taken_and_consumes_trigger_count() {
        crate::test_support::init_config();
        let managers = BattleManagers::seeded(&Fight {
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(2_000),
                    attr: Some(HeroAttribute {
                        hp: Some(2_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(2_000),
                    attr: Some(HeroAttribute {
                        hp: Some(2_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        });
        let subscriber = BuffActSubscriber {
            owner_uid: -1,
            source_uid: -1,
            buff_uid: 1_069,
            buff_id: 117200101,
            team_type: 2,
            owner_alive: true,
            amount: 1,
            key: crate::engine::event::subscription::SubscriptionKey::new(
                crate::engine::event::kind::EventKind::BeAttacked,
                DefinitionKey::new(743, "ReboundBasedOnDamage"),
            ),
            act_type: "ReboundBasedOnDamage".to_owned(),
            effect_time: 209,
            effect_condition: 0,
            args: vec![300, 0, 0],
            raw: "743#300#0,0".to_owned(),
        };
        let hit = |amount| {
            BattleEvent::Hit(HitEvent {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "Damage"),
                },
                source_uid: 10,
                target_uid: -1,
                skill_id: 1,
                amount,
                shield_absorbed: 0,
                damage_from: HurtDamageFromType::Skill,
                assassinate: false,
                ignore_riposte: false,
            })
        };

        let reflected = damage_based_rule_ops(&managers, &subscriber, &hit(1_000)).unwrap();
        assert!(matches!(
            reflected.as_slice(),
            [
                RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
                    target_uid: 10,
                    amount: 300,
                    hurt: Some(HurtInfoData {
                        buff_act_id: 743,
                        buff_uid: 1_069,
                        hurt_effect_type,
                        ..
                    }),
                    ..
                }))),
                RuleOp::Command(BattleCommand::Buff(BuffCommand::ConsumeCount(
                    BuffConsume {
                        amount: 1,
                        depleted: DepletedBuff::Remove,
                        ..
                    }
                )))
            ] if *hurt_effect_type == EffectType::Origindamage as i32
        ));
        assert!(matches!(
            damage_based_rule_ops(&managers, &subscriber, &hit(1))
                .unwrap()
                .first(),
            Some(RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
                HpLoss { amount: 1, .. }
            ))))
        ));
        assert!(!supports_damage_based(&[300, AttrId::Hp.id(), 2_000]));
    }
}
