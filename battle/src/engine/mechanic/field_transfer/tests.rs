use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::{
    entity::attr::AttrId,
    manager::{
        buff::BuffGrant,
        field::{FieldDefinition, FieldOperation, FieldThreshold},
    },
    skill::rule::{DefinitionKey, RuleDomain},
};

const ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Behavior,
    key: DefinitionKey::new(60195, "ElectricTransform"),
};

#[test]
fn transfer_commits_buff_consumption_and_field_progress_together() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    buff_id: Some(90071),
                    uid: Some(2),
                    layer: Some(6),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_field(FieldCommand {
            origin: ORIGIN,
            team: 1,
            operation: FieldOperation::DeployIfAbsent {
                definition: FieldDefinition {
                    field_id: 30001,
                    duration: 3,
                },
                create_uid: 10,
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
                ],
            },
        })
        .unwrap();

    let changes = execute(
        &mut managers,
        FieldTransferCommand {
            origin: ORIGIN,
            target_uid: 10,
            buff: BuffSelector::IdOrType(90071),
            limit: 10,
            team: 1,
        },
    )
    .unwrap();

    assert_eq!(managers.buff.buff_id_or_type_amount(10, 90071), 0);
    assert_eq!(managers.field.get(1).unwrap().progress, 6);
    assert_eq!(managers.field.round_transfer_count(1), 1);
    assert_eq!(changes.buffs[0].change.removed[0].buff.uid, Some(2));
    assert_eq!(changes.field.applied_delta, 6);
    managers.begin_round();
    assert_eq!(managers.field.round_transfer_count(1), 0);
}

#[test]
fn transfer_consumes_configured_extra_value_and_adds_it_to_progress() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![
                    BuffInfo {
                        buff_id: Some(90071),
                        uid: Some(2),
                        layer: Some(6),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(31430141),
                        uid: Some(3),
                        layer: Some(5),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_field(FieldCommand {
            origin: ORIGIN,
            team: 1,
            operation: FieldOperation::DeployIfAbsent {
                definition: FieldDefinition {
                    field_id: 30001,
                    duration: 3,
                },
                create_uid: 10,
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
                ],
            },
        })
        .unwrap();

    let changes = execute(
        &mut managers,
        FieldTransferCommand {
            origin: ORIGIN,
            target_uid: 10,
            buff: BuffSelector::IdOrType(90071),
            limit: 10,
            team: 1,
        },
    )
    .unwrap();

    assert_eq!(managers.buff.buff_id_or_type_amount(10, 90071), 0);
    assert_eq!(managers.buff.buff_id_or_type_amount(10, 31430141), 0);
    assert_eq!(managers.field.get(1).unwrap().progress, 11);
    assert_eq!(changes.buffs.len(), 2);
    assert_eq!(changes.field.applied_delta, 11);
}

#[test]
fn allied_extra_transfer_records_and_snapshots_team_attributes() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(100),
                    buffs: vec![
                        BuffInfo {
                            buff_id: Some(31430146),
                            uid: Some(4),
                            ..Default::default()
                        },
                        BuffInfo {
                            buff_id: Some(90071),
                            uid: Some(5),
                            layer: Some(6),
                            ..Default::default()
                        },
                        BuffInfo {
                            buff_id: Some(31430141),
                            uid: Some(6),
                            layer: Some(5),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    team_type: Some(1),
                    current_hp: Some(100),
                    buffs: vec![
                        BuffInfo {
                            buff_id: Some(90071),
                            uid: Some(2),
                            layer: Some(6),
                            ..Default::default()
                        },
                        BuffInfo {
                            buff_id: Some(31430141),
                            uid: Some(3),
                            layer: Some(5),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_field(FieldCommand {
            origin: ORIGIN,
            team: 1,
            operation: FieldOperation::DeployIfAbsent {
                definition: FieldDefinition {
                    field_id: 30001,
                    duration: 3,
                },
                create_uid: 10,
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
                ],
            },
        })
        .unwrap();

    execute(
        &mut managers,
        FieldTransferCommand {
            origin: ORIGIN,
            target_uid: 10,
            buff: BuffSelector::IdOrType(90071),
            limit: 10,
            team: 1,
        },
    )
    .unwrap();
    assert_eq!(managers.buff.act_value(4, 1128), 0);

    execute(
        &mut managers,
        FieldTransferCommand {
            origin: ORIGIN,
            target_uid: 20,
            buff: BuffSelector::IdOrType(90071),
            limit: 10,
            team: 1,
        },
    )
    .unwrap();
    assert_eq!(managers.buff.act_value(4, 1128), 5);

    let changes = managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: ORIGIN,
            source_uid: 10,
            target_uid: 10,
            buff_id: 31430131,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();
    let added = changes.change.added.unwrap();
    assert_eq!(added.buff.act_info[0].act_id, Some(1127));
    assert_eq!(added.buff.act_info[0].param, vec![100]);
    for uid in [10, 20] {
        assert_eq!(managers.buff.attribute_delta(uid, AttrId::DmgBonus), 100);
        assert_eq!(managers.buff.attribute_delta(uid, AttrId::CriticalDmg), 200);
    }
}
