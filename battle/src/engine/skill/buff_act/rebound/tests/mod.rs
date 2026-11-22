use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::{
    event::payload::HitEvent,
    packet::timeline::project_for_version,
    runtime::{
        executor::execute_rule_op,
        record::{FrameItem, FrameOwner, FrameTrigger, SemanticFrame},
    },
    skill::{
        rule::{CommandOrigin, DefinitionKey, RuleDomain},
        subscriber::BuffActSubscriber,
    },
};

mod layers;

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
                shield_value: Some(2_000),
                buffs: vec![BuffInfo {
                    uid: Some(77),
                    buff_id: Some(31270501),
                    from_uid: Some(10),
                    duration: Some(3),
                    ..Default::default()
                }],
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
    let mut managers = BattleManagers::seeded(&fight);
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
        resolved_damage: 100,
        shield_absorbed: 0,
        career_restraint: false,
        damage_from: HurtDamageFromType::Skill,
        share_count: 0,
        assassinate: false,
        ignore_riposte: false,
    });
    let ops = rule_ops(&managers, &subscriber, &event).unwrap();

    let [
        RuleOp::BuffFeatureMarker { target_uid: 10, .. },
        damage @ RuleOp::Command(BattleCommand::ReboundDamage(ReflectedDamage(HpDamage {
            target_uid: 10,
            amount: 1_237,
            effect_kind: DamageEffectKind::Genesis,
            ..
        }))),
        RuleOp::Command(BattleCommand::Buff(BuffCommand::ConsumeEffectCount(BuffConsume {
            amount: 1,
            depleted: DepletedBuff::Keep,
            ..
        }))),
    ] = ops.as_slice()
    else {
        panic!("expected damage settlement followed by consumption");
    };
    let mut events = Default::default();
    let outcome = execute_rule_op(&mut managers, &mut events, damage.clone()).unwrap();
    let frame = SemanticFrame {
        owner: FrameOwner::BuffAct {
            owner_uid: subscriber.owner_uid,
            source_uid: subscriber.source_uid,
            buff_uid: subscriber.buff_uid,
            buff_id: subscriber.buff_id,
            key: subscriber.key.definition,
        },
        trigger: FrameTrigger::Event(event),
        items: outcome
            .changes()
            .into_iter()
            .map(|change| FrameItem::Change(Box::new(change)))
            .collect(),
    };
    let steps = project_for_version(&[frame], 7).unwrap();
    let effect = steps
        .iter()
        .flat_map(|step| &step.act_effect)
        .find(|effect| effect.effect_type == Some(EffectType::Origindamage as i32))
        .unwrap();
    let hurt = effect.hurt_info.as_ref().unwrap();
    assert_eq!(hurt.damage, Some(1_237));
    assert_eq!(hurt.reduce_hp.unwrap_or_default(), 0);
    let absorption: serde_json::Value =
        serde_json::from_str(hurt.absorb_hurt_param.as_deref().unwrap()).unwrap();
    assert_eq!(absorption["reduceShieldBuffMap"], "77#1237");
    assert_eq!(managers.hp.current(10), 2_000);
    assert_eq!(managers.hp.shield(10), 763);

    managers.hp.set_shield(10, 0);
    managers.hp.lose(10, 1_999, 0).unwrap();
    let lethal = execute_rule_op(&mut managers, &mut events, damage.clone()).unwrap();
    assert!(!lethal.changes().is_empty());
    assert_eq!(managers.hp.current(10), 0);
    let after_death = execute_rule_op(&mut managers, &mut events, damage.clone()).unwrap();
    assert!(after_death.changes().is_empty());

    let RuleOp::Command(BattleCommand::ReboundDamage(mut invalid)) = damage.clone() else {
        unreachable!();
    };
    invalid.0.target_uid = 999;
    assert!(matches!(
        invalid.settle(&mut managers),
        Err(HpCommandError::MissingTarget(999))
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
        resolved_damage: 100,
        shield_absorbed: 0,
        career_restraint: false,
        damage_from: HurtDamageFromType::Skill,
        share_count: 0,
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
            resolved_damage: amount,
            shield_absorbed: 0,
            career_restraint: false,
            damage_from: HurtDamageFromType::Skill,
            share_count: 0,
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
