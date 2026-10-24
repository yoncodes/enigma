use super::*;

#[test]
fn grant_plan_removes_incoming_exclusions_before_add() {
    crate::test_support::init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(100001),
                    buff_id: Some(530000111),
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
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: -1,
                target_uid: -1,
                buff_id: 530000417,
                amount: Some(0),
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();

    assert_eq!(changes.change.removed.len(), 1);
    assert_eq!(changes.change.removed[0].buff.uid, Some(100001));
    assert_eq!(
        changes.change.added.as_ref().unwrap().buff.buff_id,
        Some(530000417)
    );
    assert!(!manager.has_buff_id(-1, 530000111));
    assert!(manager.has_buff_id(-1, 530000417));
}

#[test]
fn counted_refresh_only_reserves_children_during_initial_materialization() {
    crate::test_support::init_config();
    let fight = Fight {
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
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };
    let grant = |occurrences| {
        BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 10,
            target_uid: 10,
            buff_id: 30631,
            amount: None,
            occurrences,
            child_uid_reservations: 0,
        })
    };

    manager.execute(&HpManager::default(), grant(1)).unwrap();
    let plan = manager.plan(&HpManager::default(), grant(2)).unwrap();
    let refresh = grant_plan(&plan);
    assert_eq!(refresh.action, GrantAction::RefreshCount);
    assert!(refresh.uid.is_none());
    assert!(refresh.pre_add_uids.is_empty());
    assert!(refresh.post_add_uids.is_empty());
    assert_eq!(
        refresh
            .refresh_uids
            .iter()
            .map(|uid| uid.uid)
            .collect::<Vec<_>>(),
        Vec::<i64>::new()
    );
    let changes = manager.commit(&HpManager::default(), plan);
    assert_eq!(changes.change.refreshed.len(), 2);
    assert_eq!(changes.change.refreshed[1].after.count, Some(3));

    let next = manager.add(&HpManager::default(), 10, 10, 101, 0).unwrap();
    assert_eq!(next.buff.uid, Some(4));

    let mut manager = BuffManager::default();
    manager.seed(&fight);
    let plan = manager.plan(&HpManager::default(), grant(2)).unwrap();
    let add = grant_plan(&plan);
    assert_eq!(add.uid.unwrap().uid, 2);
    assert_eq!(
        add.refresh_uids
            .iter()
            .map(|uid| uid.uid)
            .collect::<Vec<_>>(),
        vec![3]
    );
    let changes = manager.commit(&HpManager::default(), plan);
    assert_eq!(changes.change.added.as_ref().unwrap().buff.count, Some(1));
    assert_eq!(changes.change.refreshed[0].after.count, Some(2));
    let next = manager.add(&HpManager::default(), 10, 10, 101, 0).unwrap();
    assert_eq!(next.buff.uid, Some(5));
}

#[test]
fn existing_count_refresh_does_not_shift_the_next_layered_buff_uid() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![
                    BuffInfo {
                        uid: Some(19),
                        buff_id: Some(30631),
                        count: Some(2),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(28),
                        buff_id: Some(101),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };

    let plan = manager
        .plan(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin,
                source_uid: 10,
                target_uid: 10,
                buff_id: 30631,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();
    let refresh = grant_plan(&plan);
    assert_eq!(refresh.action, GrantAction::RefreshCount);
    assert!(refresh.uid.is_none());
    assert!(refresh.pre_add_uids.is_empty());
    assert!(refresh.post_add_uids.is_empty());
    manager.commit(&HpManager::default(), plan);
    let layered_plan = manager
        .plan(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin,
                source_uid: 10,
                target_uid: 10,
                buff_id: 434425,
                amount: None,
                occurrences: 3,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();
    assert_eq!(grant_plan(&layered_plan).uid.unwrap().uid, 30);
    let layered = manager.commit(&HpManager::default(), layered_plan);

    assert_eq!(layered.change.added.unwrap().buff.uid, Some(30));
}

#[test]
fn existing_refresh_count_reserves_the_next_normal_uid() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                buffs: vec![
                    BuffInfo {
                        uid: Some(1039),
                        buff_id: Some(301),
                        count: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(1049),
                        buff_id: Some(999),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    manager
        .execute(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id: 301,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();
    let next = manager.add(&HpManager::default(), 10, 10, 101, 0).unwrap();

    assert_eq!(next.buff.uid, Some(1053));
}

#[test]
fn explicit_count_refresh_reuses_the_existing_instance_uid() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                buffs: vec![
                    BuffInfo {
                        uid: Some(1263),
                        buff_id: Some(31440113),
                        count: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(1518),
                        buff_id: Some(101),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    let plan = manager
        .plan(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id: 31440113,
                amount: Some(3),
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();
    assert_eq!(
        grant_plan(&plan)
            .refresh_uids
            .iter()
            .map(|uid| uid.uid)
            .collect::<Vec<_>>(),
        [1520, 1522]
    );
    let changes = manager.commit(&HpManager::default(), plan);
    assert_eq!(changes.change.refreshed.len(), 3);
    assert_eq!(changes.change.refreshed[2].after.count, Some(4));
    let next = manager
        .add(&HpManager::default(), 10, 10, 70015, 0)
        .unwrap();
    assert_eq!(next.buff.uid, Some(1524));
}

#[test]
fn counted_storage_refresh_reserves_child_uids() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1237),
                    buff_id: Some(31130123),
                    count: Some(1),
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
    manager
        .execute(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(928, "AddToTarget"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id: 31130123,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();
    manager
        .execute(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(928, "AddToTarget"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id: 31130123,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();
    let child = manager
        .execute(
            &HpManager::default(),
            BuffCommand::GrantChild(BuffGrantChild {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(893, "EmitterEnergyAddBuff"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id: 31080152,
                amount: None,
                params: None,
                act_info: None,
            }),
        )
        .unwrap();

    assert_eq!(child.change.added.unwrap().buff.uid, Some(1240));
}

#[test]
fn action_consumed_charges_are_stored_as_separate_instances() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    let first = manager
        .add(&HpManager::default(), 10, 10, 430111, 0)
        .unwrap();
    let second = manager
        .add(&HpManager::default(), 10, 10, 430111, 0)
        .unwrap();

    assert_ne!(first.buff.uid, second.buff.uid);
    assert_eq!(manager.buff_id_amount(10, 430111), 2);
}

#[test]
fn shared_group_capacity_evicts_the_oldest_uid_before_the_new_copy() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                buffs: vec![
                    BuffInfo {
                        uid: Some(1078),
                        buff_id: Some(130100112),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(1105),
                        buff_id: Some(130100122),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(1120),
                        buff_id: Some(130100122),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(1167),
                        buff_id: Some(130100122),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

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
                buff_id: 130100112,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();

    assert!(manager.snapshot(-1, 1078).is_none());
    assert_eq!(changes.change.removed.len(), 1);
    assert_eq!(changes.change.removed[0].buff.uid, Some(1078));
    assert_eq!(
        changes.change.removed[0].delete_reason,
        Some(BuffDeleteReason::Overflow)
    );
    assert!(changes.change.added.is_some());

    let effects = crate::engine::packet::effect::EffectPacket::recorded_buff_changes(&changes);
    assert_eq!(
        effects
            .iter()
            .take(3)
            .map(|effect| effect.effect_type)
            .collect::<Vec<_>>(),
        vec![
            Some(sonettobuf::effect_type_enum::EffectType::Buffdelreason as i32),
            Some(sonettobuf::effect_type_enum::EffectType::Buffdel as i32),
            Some(sonettobuf::effect_type_enum::EffectType::Buffadd as i32),
        ]
    );
    assert_eq!(effects[0].effect_num, Some(1));
    assert_eq!(effects[0].reserve_id, Some(1078));
}

#[test]
fn reserved_grant_uid_is_claimed_after_intervening_allocations() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(124),
                    buff_id: Some(101),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let origin = CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: DefinitionKey::new(1024, "MonitorContinueChannel"),
    };
    let grant = |buff_id| {
        BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 10,
            target_uid: 10,
            buff_id,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };

    let reservation = manager
        .execute(
            &HpManager::default(),
            BuffCommand::ReserveGrantUid(BuffGrantUidReservation {
                origin,
                target_uid: 10,
                buff_id: 31260161,
            }),
        )
        .unwrap();
    assert!(reservation.change.added.is_none());

    let intervening = manager
        .execute(&HpManager::default(), grant(31260111))
        .unwrap();
    let reserved = manager
        .execute(&HpManager::default(), grant(31260161))
        .unwrap();

    assert_eq!(intervening.change.added.unwrap().buff.uid, Some(127));
    assert_eq!(reserved.change.added.unwrap().buff.uid, Some(125));
}

#[test]
fn normal_uid_grant_keeps_layered_storage_on_the_root_lane() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1107),
                    buff_id: Some(31280112),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let grant = BuffGrant {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(1, "AddBuff"),
        },
        source_uid: 10,
        target_uid: 10,
        buff_id: 31430151,
        amount: Some(1),
        occurrences: 1,
        child_uid_reservations: 0,
    };

    let default_plan = manager
        .plan(&HpManager::default(), BuffCommand::Grant(grant))
        .unwrap();
    let normal_plan = manager
        .plan(
            &HpManager::default(),
            BuffCommand::GrantRelated(RelatedBuffGrant {
                grant,
                relation: BuffGrantRelation::Normal,
            }),
        )
        .unwrap();

    assert_eq!(grant_plan(&default_plan).uid.unwrap().uid, 1108);
    assert_eq!(grant_plan(&normal_plan).uid.unwrap().uid, 1109);
}
