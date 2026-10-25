use super::*;

#[test]
fn attacker_assist_uses_its_resolved_team_uid_lane() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(6),
        attacker: Some(FightTeam {
            assist_boss: Some(FightEntityInfo {
                uid: Some(-1),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(900),
                    buff_id: Some(999_999_999),
                    from_uid: Some(-1),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    let changes = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: -1,
                target_uid: -1,
                buff_id: 70015,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();

    assert_eq!(manager.team_type(-1), Some(1));
    assert_eq!(
        manager.team_type(crate::engine::fight::rules::ATTACKER_SIDE_UID),
        Some(1)
    );
    assert_eq!(
        manager.team_type(crate::engine::fight::rules::DEFENDER_SIDE_UID),
        Some(2)
    );
    assert_eq!(manager.team_type(404), None);
    assert_eq!(changes.change.added.unwrap().buff.uid, Some(902));
}

#[test]
fn grant_reservations_advance_the_uid_lane_even_when_the_buff_refreshes() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(36),
                    buff_id: Some(31280113),
                    duration: Some(0),
                    layer: Some(70),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();

    manager.seed(&fight);
    let changes = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(60222, "ConsumeCardAddBuff"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id: 31280113,
                amount: Some(1),
                occurrences: 1,
                child_uid_reservations: 1,
            }),
        )
        .unwrap();
    let following = manager
        .add(&HpManager::default(), 10, 10, 31080141, 0)
        .unwrap();

    assert_eq!(
        changes.change.refreshed.len(),
        1,
        "unexpected grant result: {:#?}",
        changes.change
    );
    assert_eq!(changes.change.refreshed[0].after.uid, Some(36));
    assert_eq!(changes.change.refreshed[0].after.layer, Some(71));
    // Refreshing an existing layer only consumes the configured reservation.
    assert_eq!(following.buff.uid, Some(39));
}

#[test]
fn heat_scale_counter_reapply_keeps_state_and_consumes_one_normal_uid() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);
    let grant = || {
        BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: 10,
            target_uid: 10,
            buff_id: 30810301,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };
    let initial = manager
        .execute(&HpManager::default(), grant())
        .unwrap()
        .change
        .added
        .expect("initial heat-scale counter");
    let initial_uid = initial.buff.uid.expect("counter uid");
    manager
        .execute(
            &HpManager::default(),
            BuffCommand::SetState(BuffSetState {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(1062, "HeatScaleDecrCounter"),
                },
                target_uid: 10,
                buff_uid: initial_uid,
                ex_info: Some(37),
                params: None,
                act_info: None,
            }),
        )
        .unwrap();

    let changes = manager.execute(&HpManager::default(), grant()).unwrap();
    let following = manager
        .add(&HpManager::default(), 10, 10, 31340007, 0)
        .unwrap();

    assert_eq!(initial_uid, 1002);
    assert!(changes.change.added.is_none());
    assert!(changes.change.removed.is_empty());
    assert!(changes.change.refreshed.is_empty());
    assert_eq!(manager.snapshot(10, initial_uid).unwrap().ex_info, Some(37));
    assert_eq!(following.buff.uid, Some(1006));
}

#[test]
fn include_type_seven_does_not_reserve_hidden_uids() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    let change = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id: 530000412,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();

    assert_eq!(change.change.added.unwrap().buff.uid, Some(2));
}

#[test]
fn hidden_sixteen_layer_buff_reserves_a_child_before_explicit_first_grant() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(139),
                    buff_id: Some(101),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: DefinitionKey::new(1019, "LostHpCountAddBuff"),
    };

    let first = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin,
                source_uid: 10,
                target_uid: 10,
                buff_id: 434811,
                amount: Some(2),
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();
    let next = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin,
                source_uid: 10,
                target_uid: 10,
                buff_id: 435631,
                amount: Some(2),
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();

    assert_eq!(first.change.added.unwrap().buff.uid, Some(141));
    assert_eq!(next.change.added.unwrap().buff.uid, Some(142));
}

#[test]
fn preceding_child_allocation_satisfies_the_explicit_layer_reservation() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1171),
                    buff_id: Some(101),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };

    let marker = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin,
                source_uid: 10,
                target_uid: 10,
                buff_id: 31250191,
                amount: Some(1),
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();
    let layered = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin,
                source_uid: 10,
                target_uid: 10,
                buff_id: 434811,
                amount: Some(1),
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();

    assert_eq!(marker.change.added.unwrap().buff.uid, Some(1172));
    assert_eq!(layered.change.added.unwrap().buff.uid, Some(1173));
}
