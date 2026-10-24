use sonettobuf::{Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::{
    manager::hp::{DamageEffectKind, DamageRecord, HurtDamageFromType, HurtInfoData},
    skill::rule::{DefinitionKey, RuleDomain},
};

#[test]
fn unique_skill_point_requires_a_known_type_and_positive_maximum() {
    assert_eq!(
        parse_unique_skill_point("4#100").unwrap(),
        (ExPointKind::DevicePower, 100)
    );

    for malformed in [
        "4",
        "common#100",
        "4#many",
        "4#100#1",
        "42#100",
        "4#0",
        "4#-1",
    ] {
        assert!(parse_unique_skill_point(malformed).is_err());
    }
}

#[test]
fn nautika_uses_rank_replaced_faith_cap() {
    crate::test_support::init_config();
    let mut fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                model_id: Some(3120),
                level: Some(180),
                ex_point_type: Some(1),
                expoint_max_add: Some(0),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = ExPointManager::default();

    manager.seed(&fight);
    let changes = manager
        .execute_command(
            ExPointCommand::Change(ExPointChange {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(20002, "AddExPoint"),
                },
                source_uid: 1,
                target_uid: 1,
                delta: 20,
                config_effect: 0,
                effect_type: 0,
            }),
            true,
            true,
        )
        .unwrap();
    manager.sync_entity(&mut fight.attacker.as_mut().unwrap().entitys[0]);

    let ExPointChanges::Value { change, .. } = changes else {
        panic!("expected value change");
    };
    assert_eq!(change.kind, ExPointKind::Faith);
    assert_eq!((change.after, change.overflow), (8, 12));
    assert_eq!(changes.events().len(), 2);
    assert_eq!(fight.attacker.unwrap().entitys[0].expoint_max_add, Some(0));
}

#[test]
fn synchronization_lifecycle_is_owned_by_the_resource_manager() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                ex_point_type: Some(ExPointKind::Synchronization.as_wire()),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = ExPointManager::default();
    manager.seed(&fight);
    let definition = SynchronizationDefinition::new([11, 12, 13], 4, 100).unwrap();

    manager
        .execute_command(
            ExPointCommand::ConfigureSynchronization(ExPointConfigureSynchronization {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(100000, "EzioProps"),
                },
                target_uid: 1,
                definition,
            }),
            true,
            true,
        )
        .unwrap();

    assert_eq!(manager.synchronization_definition(1), Some(definition));
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(100022, "EzioBigSkillCheckTimes"),
    };
    manager
        .execute_command(
            ExPointCommand::Set(ExPointSet {
                origin,
                source_uid: 1,
                target_uid: 1,
                value: 100,
                config_effect: 0,
                effect_type: 0,
            }),
            true,
            true,
        )
        .unwrap();
    for damage in [120, 180] {
        let progress_before_hit = manager.synchronization_progress(1);
        let damage = DamageRecord {
            amount: damage,
            config_effect: 100002,
            effect_kind: DamageEffectKind::Critical,
            assassinate: true,
            ignore_riposte: false,
            hurt: HurtInfoData {
                from_uid: 1,
                is_crit: true,
                career_restraint: true,
                reduce_hp: 0,
                effect_id: 11,
                skill_id: 11,
                damage_from: HurtDamageFromType::SkillEffect,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: 3,
                display_amount: None,
            },
        };
        let hit = manager
            .execute_command(
                ExPointCommand::PublishSynchronizationHit(ExPointPublishSynchronizationHit {
                    origin,
                    target_uid: 1,
                    action_target_uid: -1,
                    damage,
                }),
                true,
                true,
            )
            .unwrap();
        assert!(matches!(
            hit.events().as_slice(),
            [BattleEvent::Hit(event)]
                if event.amount == damage.amount && event.resolved_damage == damage.amount
        ));
        assert_eq!(manager.synchronization_progress(1), progress_before_hit);
        let recorded = manager
            .execute_command(
                ExPointCommand::RecordSynchronizationDamage(ExPointRecordSynchronizationDamage {
                    origin,
                    target_uid: 1,
                    action_target_uid: -1,
                    damage,
                }),
                true,
                true,
            )
            .unwrap();
        assert!(recorded.events().is_empty());
        manager
            .execute_command(
                ExPointCommand::RecordSynchronizationAction(ExPointRecordSynchronizationAction {
                    origin,
                    target_uid: 1,
                    action_target_uid: -1,
                }),
                true,
                true,
            )
            .unwrap();
    }
    assert_eq!(
        manager.synchronization_progress(1),
        Some(SynchronizationProgress {
            completed_actions: 2,
            target_uid: -1,
            total_damage: 300,
        })
    );

    let restarted = manager
        .execute_command(
            ExPointCommand::RestartSynchronization(ExPointRestartSynchronization {
                origin,
                target_uid: 1,
            }),
            true,
            true,
        )
        .unwrap();
    assert_eq!(manager.get(1), 100);
    assert_eq!(
        manager.synchronization_progress(1),
        Some(SynchronizationProgress::default())
    );
    assert!(restarted.events().is_empty());

    let completed = manager
        .execute_command(
            ExPointCommand::CompleteSynchronization(ExPointCompleteSynchronization {
                origin,
                target_uid: 1,
            }),
            true,
            true,
        )
        .unwrap();
    assert_eq!(manager.get(1), 0);
    assert!(completed.events().is_empty());
}

#[test]
fn device_power_keeps_its_wire_kind_and_configured_cap() {
    crate::test_support::init_config();
    let mut fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                model_id: Some(3149),
                level: Some(1),
                ex_point: Some(70),
                ex_point_type: Some(ExPointKind::DevicePower.as_wire()),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = ExPointManager::default();

    manager.seed(&fight);
    let change = manager.add(1, 1, 40, 0);
    manager.sync_entity(&mut fight.attacker.as_mut().unwrap().entitys[0]);

    assert_eq!(change.kind, ExPointKind::DevicePower);
    assert_eq!((change.after, change.overflow), (100, 10));
    assert_eq!(
        fight.attacker.unwrap().entitys[0].ex_point_type,
        Some(ExPointKind::DevicePower.as_wire())
    );
}
