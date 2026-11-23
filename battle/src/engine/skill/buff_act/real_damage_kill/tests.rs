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
