use super::*;

fn manager_with_restored_layer(layer: i32) -> BuffManager {
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                buffs: vec![BuffInfo {
                    uid: None,
                    buff_id: Some(30860113),
                    from_uid: Some(10),
                    layer: Some(layer),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    manager
}

#[test]
fn partially_capped_aggregate_stack_update_consumes_one_attempt_uid() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![
                    BuffInfo {
                        uid: Some(1023),
                        buff_id: Some(90071),
                        layer: Some(28),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(1058),
                        buff_id: Some(777),
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

    let refresh = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin,
                source_uid: 10,
                target_uid: 10,
                buff_id: 90071,
                amount: Some(2),
                occurrences: 3,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();
    let following = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin,
                source_uid: 10,
                target_uid: 10,
                buff_id: 101,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();

    assert_eq!(refresh.change.refreshed[0].after.layer, Some(30));
    assert_eq!(following.change.added.unwrap().buff.uid, Some(1061));
}

#[test]
fn layered_refresh_plans_update_and_capped_noop() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };
    let grant = |source_uid, amount| {
        BuffCommand::Grant(BuffGrant {
            origin,
            source_uid,
            target_uid: 10,
            buff_id: 90071,
            amount: Some(amount),
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };

    manager
        .execute(&HpManager::default(), grant(10, 20))
        .unwrap();
    let update = manager.plan(&HpManager::default(), grant(11, 10)).unwrap();
    let update_plan = grant_plan(&update);
    assert!(matches!(
        update_plan.layer_refresh,
        Some(LayerRefreshPlan::Update { next_layer: 30, .. })
    ));
    let update = manager.commit(&HpManager::default(), update);
    assert_eq!(update.change.refreshed[0].after.layer, Some(30));
    assert_eq!(update.change.refreshed[0].after.from_uid, Some(10));

    let capped = manager.plan(&HpManager::default(), grant(11, 2)).unwrap();
    let capped_plan = grant_plan(&capped);
    assert_eq!(capped_plan.layer_refresh, Some(LayerRefreshPlan::NoChange));
    let capped = manager.commit(&HpManager::default(), capped);
    assert!(capped.change.refreshed.is_empty());

    let next = manager.add(&HpManager::default(), 10, 10, 101, 0).unwrap();
    assert_eq!(next.buff.uid, Some(4));
}

#[test]
fn version_seven_capped_layer_refresh_consumes_its_attempt_uid() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
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
    });
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };
    let grant = |amount| {
        BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 10,
            target_uid: 10,
            buff_id: 90071,
            amount: Some(amount),
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };

    manager.execute(&HpManager::default(), grant(30)).unwrap();
    let capped = manager.plan(&HpManager::default(), grant(2)).unwrap();
    assert_eq!(grant_plan(&capped).layer_refresh_uid.unwrap().uid, 1003);
    manager.commit(&HpManager::default(), capped);

    let next = manager.add(&HpManager::default(), 10, 10, 101, 0).unwrap();
    assert_eq!(next.buff.uid, Some(1005));
}

#[test]
fn version_seven_reserves_only_the_first_capped_attempt_per_carrier_in_a_transaction() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: (10..14)
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    team_type: Some(1),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(1100 + uid),
                        buff_id: Some(434121),
                        from_uid: Some(13),
                        layer: Some(12),
                        duration: Some(3),
                        ..Default::default()
                    }],
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    });
    let grant = |target_uid| {
        BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: 13,
            target_uid,
            buff_id: 434121,
            amount: Some(1),
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };

    manager.begin_transaction();
    for target_uid in 10..14 {
        let plan = manager
            .plan(&HpManager::default(), grant(target_uid))
            .unwrap();
        assert!(grant_plan(&plan).layer_refresh_uid.is_some());
        manager.commit(&HpManager::default(), plan);
    }
    for target_uid in 10..14 {
        let plan = manager
            .plan(&HpManager::default(), grant(target_uid))
            .unwrap();
        assert!(grant_plan(&plan).layer_refresh_uid.is_none());
        manager.commit(&HpManager::default(), plan);
    }
    manager.end_transaction();
}

#[test]
fn configured_max_layer_modifier_raises_its_target_buff_cap_for_allies() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(1),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };
    let grant = |target_uid, buff_id, amount| {
        BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 10,
            target_uid,
            buff_id,
            amount: Some(amount),
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };

    let mut baseline = BuffManager::default();
    baseline.seed(&fight);
    let base = baseline
        .execute(&HpManager::default(), grant(11, 31430141, 10))
        .unwrap();
    assert_eq!(base.change.added.unwrap().buff.layer, Some(5));

    let mut modified = BuffManager::default();
    modified.seed(&fight);
    modified
        .execute(&HpManager::default(), grant(10, 31430154, 1))
        .unwrap();
    let raised = modified
        .execute(&HpManager::default(), grant(11, 31430141, 10))
        .unwrap();
    assert_eq!(raised.change.added.unwrap().buff.layer, Some(10));
}

#[test]
fn direct_layer_refresh_uses_only_the_manager_owned_update_uid() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    let command = || {
        BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(1051, "CrystalAddBuff"),
            },
            source_uid: 10,
            target_uid: 10,
            buff_id: 31_340_001,
            amount: Some(1),
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };

    manager.execute(&HpManager::default(), command()).unwrap();
    let refresh = manager.plan(&HpManager::default(), command()).unwrap();
    assert!(grant_plan(&refresh).pre_add_uids.is_empty());
    assert_eq!(grant_plan(&refresh).layer_refresh_uid.unwrap().uid, 3);
    manager.commit(&HpManager::default(), refresh);

    let next = manager.add(&HpManager::default(), 10, 10, 101, 0).unwrap();
    assert_eq!(next.buff.uid, Some(5));
}

#[test]
fn first_mutation_promotes_restored_layered_state_to_a_managed_uid() {
    crate::test_support::init_config();
    let mut manager = manager_with_restored_layer(1);

    let changes = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id: 30860113,
                amount: Some(2),
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();

    let [refresh] = changes.change.refreshed.as_slice() else {
        panic!("expected the restored state to become a managed layered buff")
    };
    assert_eq!(refresh.before.uid, None);
    assert_eq!(refresh.before.layer, Some(1));
    let promoted_uid = refresh.after.uid.expect("managed buff uid");
    assert!(promoted_uid > 0);
    assert_eq!(refresh.after.layer, Some(3));
    assert_eq!(manager.snapshot(10, promoted_uid).unwrap().layer, Some(3));
}

#[test]
fn repeated_restored_grants_preserve_configured_layer_progress() {
    crate::test_support::init_config();
    let mut manager = manager_with_restored_layer(2);

    let changes = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id: 30860113,
                amount: Some(2),
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();

    let [refresh] = changes.change.refreshed.as_slice() else {
        panic!("expected restored grants to remain one layered state")
    };
    assert_eq!(refresh.before.layer, Some(2));
    assert_eq!(refresh.after.layer, Some(4));
}

#[test]
fn zero_count_special_layer_refresh_uses_the_manager_owned_update_uid() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    buff_id: Some(30070211),
                    uid: Some(1009),
                    from_uid: Some(10),
                    layer: Some(4),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let refresh = manager
        .plan(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id: 30070211,
                amount: None,
                occurrences: 2,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();

    assert_eq!(grant_plan(&refresh).layer_refresh_uid.unwrap().uid, 1010);
    manager.commit(&HpManager::default(), refresh);
    assert_eq!(manager.snapshot(10, 1009).unwrap().layer, Some(6));
    let following = manager.add(&HpManager::default(), 10, 10, 101, 0).unwrap();
    assert_eq!(following.buff.uid, Some(1012));
}

#[test]
fn repeated_direct_layer_grant_does_not_use_stacked_instance_reservations() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    let command = BuffCommand::Grant(BuffGrant {
        origin: CommandOrigin {
            domain: RuleDomain::BuffAct,
            key: DefinitionKey::new(748, "UseDamageSkillAddToTarget"),
        },
        source_uid: 10,
        target_uid: 10,
        buff_id: 4_150_001,
        amount: None,
        occurrences: 6,
        child_uid_reservations: 0,
    });

    let planned = manager.plan(&HpManager::default(), command).unwrap();
    let plan = grant_plan(&planned);
    assert!(plan.pre_add_uids.is_empty());
    assert!(plan.post_add_uids.is_empty());
    assert_eq!(plan.uid.unwrap().uid, 2);

    manager.commit(&HpManager::default(), planned);
    let next = manager.add(&HpManager::default(), 10, 10, 101, 0).unwrap();
    assert_eq!(next.buff.uid, Some(4));
}

#[test]
fn rejected_stacked_grant_advances_capped_siblings_in_the_same_transaction() {
    crate::test_support::init_config();
    let facade = |uid, owner_uid| BuffInfo {
        uid: Some(uid),
        buff_id: Some(530_000_111),
        from_uid: Some(owner_uid),
        layer: Some(3),
        ..Default::default()
    };
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    team_type: Some(2),
                    buffs: vec![BuffInfo {
                        uid: Some(100_004),
                        buff_id: Some(530_000_417),
                        from_uid: Some(-1),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    team_type: Some(2),
                    buffs: vec![facade(100_007, -2)],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-3),
                    team_type: Some(2),
                    buffs: vec![facade(100_008, -3)],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);
    let grant = |target_uid| {
        BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: target_uid,
            target_uid,
            buff_id: 530_000_111,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };

    manager.begin_transaction();
    manager.execute(&HpManager::default(), grant(-1)).unwrap();
    for (target_uid, expected_uid) in [(-2, 100_010), (-3, 100_011)] {
        let planned = manager
            .plan(&HpManager::default(), grant(target_uid))
            .unwrap();
        assert_eq!(
            grant_plan(&planned).layer_refresh_uid.unwrap().uid,
            expected_uid
        );
        manager.commit(&HpManager::default(), planned);
    }
    manager.end_transaction();

    let next = manager.add(&HpManager::default(), -1, -1, 101, 0).unwrap();
    assert_eq!(next.buff.uid, Some(100_013));
}
