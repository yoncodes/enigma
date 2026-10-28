use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, PowerInfo};

use super::*;
use crate::engine::{
    event::kind::EventKind,
    manager::{
        BattleManagers,
        gauge::{GaugeCommand, GaugeOperation},
    },
    runtime::{
        determinism::RoundDeterminism,
        drain::{self, ReactionLane},
    },
    skill::{
        action::{SkillActionEvent, SkillPhase},
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
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(2_240_000),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    team_type: Some(1),
                    current_hp: Some(1_000),
                    buffs: vec![BuffInfo {
                        uid: Some(21),
                        buff_id: Some(31410407),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
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
    let origin = CommandOrigin {
        domain: RuleDomain::Lifecycle,
        key: DefinitionKey::new(0, "TestSetup"),
    };
    for key in [
        impromptu::team_energy_key(1),
        impromptu::inspiration_key(emitter::UID),
    ] {
        managers
            .execute_gauge(GaugeCommand::new(
                origin,
                key,
                GaugeOperation::Enable { max: None },
            ))
            .unwrap();
    }
    managers
}

fn cast(mode: SkillExecutionMode) -> BattleEvent {
    BattleEvent::SkillAction(SkillActionEvent {
        source_uid: 11,
        skill_id: 1,
        target_uid: -1,
        target_uids: vec![-1],
        attacked_target_uids: vec![-1],
        phase: SkillPhase::HitPassives,
        skill_slot: 1,
        is_attack: true,
        rank: 1,
        skill_type: 0,
        effect_tag: 0,
        assassinate: false,
        ignore_riposte: false,
        damage_amount: 1,
        kill_count: 0,
        crit_count: 0,
        guard_break_count: 0,
        additional_moxie: 0,
        extra_skill_kind: 0,
        mode,
        teammate_injury_count: 0,
        teammate_injury_count_not_reset: 0,
        team_injury_count_round: 0,
        card_enchants: Vec::new(),
        buff_additions: Vec::new(),
    })
}

#[test]
fn grammar_keeps_cost_and_inspiration_distinct() {
    assert!(supports(&[2, 2]));
    assert!(supports(&[3, 4]));
    assert!(!supports(&[2]));
    assert!(!supports(&[2, 0]));

    let wire = super::super::registry::find(1082, "UseSkillConsumeFromAddEmitterEnergy")
        .unwrap()
        .wire
        .unwrap();
    assert_eq!(
        wire.markers(super::super::wire::WirePhase::Add),
        &[sonettobuf::effect_type_enum::EffectType::None as i32]
    );
    assert!(
        wire.markers(super::super::wire::WirePhase::Static)
            .is_empty()
    );
    assert!(
        wire.markers(super::super::wire::WirePhase::Refresh)
            .is_empty()
    );
}

#[test]
fn active_holder_cast_spends_source_eureka_then_adds_inspiration() {
    let fight = fight(4);
    let pool = TargetPool::from_fight(&fight);
    let mut managers = managers(4);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [31410407]);
    let subscriber = subscriber::for_active_buffs(&managers, EventKind::SkillCast)
        .into_iter()
        .find(|subscriber| {
            super::super::subscriber_is_kind(
                subscriber,
                BuffActKind::UseSkillConsumeFromAddEmitterEnergy,
            )
        })
        .unwrap();

    assert!(matches!(
        rule_ops(
            &managers,
            &subscriber,
            &cast(SkillExecutionMode::Active)
        )
        .unwrap()
        .as_slice(),
        [
            RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(EurekaChange {
                source_uid: 10,
                target_uid: 10,
                delta: -2,
                ..
            }))),
            RuleOp::Command(BattleCommand::Gauge(GaugeCommand {
                key,
                operation: GaugeOperation::ChangeValue { delta: 2 },
                ..
            }))
        ] if *key == impromptu::inspiration_key(emitter::UID)
    ));

    let mut stronger = subscriber.clone();
    stronger.args = vec![3, 4];
    assert!(matches!(
        rule_ops(&managers, &stronger, &cast(SkillExecutionMode::Active))
            .unwrap()
            .as_slice(),
        [
            RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(EurekaChange {
                delta: -3,
                ..
            }))),
            RuleOp::Command(BattleCommand::Gauge(GaugeCommand {
                operation: GaugeOperation::ChangeValue { delta: 4 },
                ..
            }))
        ]
    ));

    drain::run_group_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        cast(SkillExecutionMode::Active),
        ReactionLane::BuffActs,
        Some(&[11]),
    )
    .unwrap();

    assert_eq!(managers.eureka.get(10, EUREKA_RESOURCE_ID).current, 2);
    assert_eq!(
        managers
            .gauge
            .get(impromptu::team_energy_key(1))
            .unwrap()
            .current,
        2
    );
    assert_eq!(
        managers
            .gauge
            .get(impromptu::inspiration_key(emitter::UID))
            .unwrap()
            .current,
        2
    );
}

#[test]
fn nested_cast_or_insufficient_eureka_does_nothing() {
    let fight = fight(1);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [31410407]);

    for (mode, eureka) in [
        (SkillExecutionMode::Active, 1),
        (SkillExecutionMode::Nested, 4),
    ] {
        let mut managers = managers(eureka);
        drain::run_group_event(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            cast(mode),
            ReactionLane::BuffActs,
            Some(&[11]),
        )
        .unwrap();

        assert_eq!(managers.eureka.get(10, EUREKA_RESOURCE_ID).current, eureka);
        assert_eq!(
            managers
                .gauge
                .get(impromptu::inspiration_key(emitter::UID))
                .unwrap()
                .current,
            0
        );
        assert_eq!(
            managers
                .gauge
                .get(impromptu::team_energy_key(1))
                .unwrap()
                .current,
            0
        );
    }
}
