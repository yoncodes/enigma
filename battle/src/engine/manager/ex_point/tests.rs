use sonettobuf::{Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};

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
fn synchronization_definition_is_owned_by_the_resource_manager() {
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
    for damage in [120, 180] {
        manager
            .execute_command(
                ExPointCommand::RecordSynchronizationAction(ExPointRecordSynchronizationAction {
                    origin,
                    target_uid: 1,
                    action_target_uid: -1,
                    damage,
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
