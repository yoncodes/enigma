use super::*;
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};

const ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Behavior,
    key: DefinitionKey::new(50019, "AddMagicCircle"),
};

#[test]
fn first_deployer_owns_unclamped_progress_and_configured_level_resolution() {
    let mut manager = FieldManager::default();
    let deploy = |create_uid| FieldCommand {
        origin: ORIGIN,
        team: 1,
        operation: FieldOperation::DeployIfAbsent {
            definition: FieldDefinition {
                field_id: 30001,
                duration: 3,
            },
            create_uid,
            initial_level: 1,
            thresholds: vec![
                FieldThreshold {
                    level: 1,
                    progress: 0,
                    definition: FieldDefinition {
                        field_id: 30001,
                        duration: 3,
                    },
                },
                FieldThreshold {
                    level: 2,
                    progress: 90,
                    definition: FieldDefinition {
                        field_id: 30002,
                        duration: 3,
                    },
                },
                FieldThreshold {
                    level: 3,
                    progress: 120,
                    definition: FieldDefinition {
                        field_id: 30003,
                        duration: 2,
                    },
                },
            ],
        },
    };
    manager.execute_command(deploy(10)).unwrap();
    manager.execute_command(deploy(11)).unwrap();
    assert_eq!(manager.get(1).unwrap().create_uid, 10);
    assert_eq!(manager.get(1).unwrap().next_upgrade_progress, 90);

    manager
        .execute_command(FieldCommand {
            origin: ORIGIN,
            team: 1,
            operation: FieldOperation::ChangeProgress { delta: 110 },
        })
        .unwrap();
    assert_eq!(manager.get(1).unwrap().progress, 110);

    let thresholds = vec![
        FieldThreshold {
            level: 2,
            progress: 90,
            definition: FieldDefinition {
                field_id: 30002,
                duration: 3,
            },
        },
        FieldThreshold {
            level: 3,
            progress: 120,
            definition: FieldDefinition {
                field_id: 30003,
                duration: 2,
            },
        },
    ];
    let resolved = manager
        .execute_command(FieldCommand {
            origin: ORIGIN,
            team: 1,
            operation: FieldOperation::ResolveLevel {
                thresholds: thresholds.clone(),
            },
        })
        .unwrap();
    assert_eq!(resolved.after.unwrap().level, 2);
    assert_eq!(resolved.after.unwrap().definition.field_id, 30002);
    assert_eq!(resolved.after.unwrap().next_upgrade_progress, 120);
    assert_eq!(resolved.overflow, 0);

    let overflow = manager
        .execute_command(FieldCommand {
            origin: ORIGIN,
            team: 1,
            operation: FieldOperation::ChangeProgress { delta: 20 },
        })
        .unwrap();
    assert_eq!(overflow.after.unwrap().progress, 130);
    assert_eq!(overflow.overflow, 10);

    manager
        .execute_command(FieldCommand {
            origin: ORIGIN,
            team: 1,
            operation: FieldOperation::ChangeDuration { delta: -1 },
        })
        .unwrap();
    manager
        .execute_command(FieldCommand {
            origin: ORIGIN,
            team: 1,
            operation: FieldOperation::ResolveLevel { thresholds },
        })
        .unwrap();
    assert_eq!(manager.get(1).unwrap().definition.duration, 2);
}

#[test]
fn active_field_attributes_are_read_from_the_authoritative_team_state() {
    crate::test_support::init_config();
    let pool = TargetPool::from_fight(&sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(10),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(-1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let mut manager = FieldManager::default();
    manager
        .execute_command(FieldCommand {
            origin: ORIGIN,
            team: 1,
            operation: FieldOperation::DeployIfAbsent {
                definition: FieldDefinition {
                    field_id: 30001,
                    duration: 3,
                },
                create_uid: 10,
                initial_level: 1,
                thresholds: Vec::new(),
            },
        })
        .unwrap();

    assert_eq!(
        manager.attribute_delta(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            10,
            AttrId::DmgBonus,
            &pool
        ),
        150
    );
    assert_eq!(
        manager.attribute_delta(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            -1,
            AttrId::DmgBonus,
            &pool
        ),
        0
    );
}
