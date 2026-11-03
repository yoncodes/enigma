use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, PowerInfo};

use super::*;
use crate::engine::{
    event::kind::EventKind,
    manager::{
        BattleManagers,
        gauge::{GaugeCommand, GaugeOperation},
    },
    mechanic::impromptu,
    runtime::{
        determinism::RoundDeterminism,
        drain::{self, ReactionLane},
    },
    skill::{
        effect::SkillEffectCatalog,
        rule::{CommandOrigin, DefinitionKey, RuleDomain},
        subscriber,
        target::{TargetContext, TargetPool},
    },
};

fn fight(eureka: i32) -> Fight {
    Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(1_000),
                    power_infos: vec![PowerInfo {
                        power_id: Some(EUREKA_RESOURCE_ID),
                        num: Some(eureka),
                        max: Some(5),
                    }],
                    buffs: vec![
                        BuffInfo {
                            uid: Some(20),
                            buff_id: Some(31410202),
                            from_uid: Some(10),
                            layer: Some(2),
                            ..Default::default()
                        },
                        BuffInfo {
                            uid: Some(21),
                            buff_id: Some(2240000),
                            from_uid: Some(10),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(emitter::UID),
                    team_type: Some(1),
                    current_hp: Some(1),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(1_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn managers(eureka: i32) -> BattleManagers {
    crate::test_support::init_config();
    let mut managers = BattleManagers::seeded(&fight(eureka));
    managers
        .execute_gauge(GaugeCommand::new(
            CommandOrigin {
                domain: RuleDomain::Lifecycle,
                key: DefinitionKey::new(0, "TestSetup"),
            },
            impromptu::team_energy_key(1),
            GaugeOperation::Enable { max: None },
        ))
        .unwrap();
    managers
}

#[test]
fn grammar_keeps_the_exact_emitter_variant_and_destination() {
    assert!(supports(&[1, 2, 31410203, 2]));
    assert_eq!(referenced_buff(&[1, 2, 31410203, 2]), Some(31410203));
    assert!(!supports(&[1, 2, 31410203, 1]));
    assert!(!supports(&[1, 2, 31410203]));
}

#[test]
fn round_start_spends_eureka_grants_emitter_buff_then_consumes_stack() {
    let fight = fight(4);
    let pool = TargetPool::from_fight(&fight);
    let mut managers = managers(4);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [31410202]);
    let subscriber = subscriber::for_active_buffs(&managers, EventKind::RoundStart)
        .into_iter()
        .find(|subscriber| {
            super::super::subscriber_is_kind(subscriber, BuffActKind::ConsumeCurBuffAndAuraAddBuff)
        })
        .unwrap();

    assert!(matches!(
        rule_ops(&managers, &subscriber, &BattleEvent::RoundStart)
            .unwrap()
            .as_slice(),
        [
            RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(EurekaChange {
                target_uid: 10,
                delta: -2,
                ..
            }))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                source_uid: 10,
                target_uid: emitter::UID,
                buff_id: 31410203,
                ..
            }))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                target_uid: 10,
                selector: BuffSelector::Uid(20),
                amount: 1,
                depleted: DepletedBuff::Remove,
                ..
            })))
        ]
    ));

    drain::run_group_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::RoundStart,
        ReactionLane::BuffActs,
        Some(&[10]),
    )
    .unwrap();

    assert_eq!(managers.eureka.get(10, EUREKA_RESOURCE_ID).current, 2);
    assert_eq!(managers.buff.buff_id_amount(10, 31410202), 1);
    assert!(managers.buff.has_buff_id(emitter::UID, 31410203));
    assert_eq!(
        managers
            .gauge
            .get(impromptu::team_energy_key(1))
            .unwrap()
            .current,
        2
    );

    drain::run_group_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::RoundStart,
        ReactionLane::BuffActs,
        Some(&[10]),
    )
    .unwrap();

    assert_eq!(managers.eureka.get(10, EUREKA_RESOURCE_ID).current, 0);
    assert_eq!(managers.buff.buff_id_amount(10, 31410202), 0);
    assert!(!managers.buff.has_buff_id(10, 31410202));
}

#[test]
fn insufficient_eureka_preserves_the_source_stack() {
    let fight = fight(1);
    let pool = TargetPool::from_fight(&fight);
    let mut managers = managers(1);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [31410202]);

    drain::run_group_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::RoundStart,
        ReactionLane::BuffActs,
        Some(&[10]),
    )
    .unwrap();

    assert_eq!(managers.eureka.get(10, EUREKA_RESOURCE_ID).current, 1);
    assert_eq!(managers.buff.buff_id_amount(10, 31410202), 2);
    assert!(!managers.buff.has_buff_id(emitter::UID, 31410203));
}
