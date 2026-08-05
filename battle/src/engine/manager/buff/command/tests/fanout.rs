use super::*;

#[test]
fn burn_cap_uses_each_team_aura_once() {
    crate::test_support::init_config();
    let aura = |uid| BuffInfo {
        uid: Some(uid),
        buff_id: Some(31270413),
        from_uid: Some(10),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    buffs: vec![aura(1)],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    buffs: vec![aura(2)],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);
    let result = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: -1,
                buff_id: 4150001,
                amount: Some(50),
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();

    assert_eq!(result.change.added.unwrap().buff.layer, Some(40));
}

#[test]
fn accumulate_updates_a_layer_without_echoing_the_previous_value() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(50035, "AddBuffBasedOnEnemyBurnUseCount"),
    };
    let command = || {
        BuffCommand::Accumulate(BuffGrant {
            origin,
            source_uid: 10,
            target_uid: 10,
            buff_id: 30810108,
            amount: Some(6),
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };

    manager.execute(&HpManager::default(), command()).unwrap();
    let plan = manager.plan(&HpManager::default(), command()).unwrap();
    assert!(matches!(
        grant_plan(&plan).layer_refresh,
        Some(LayerRefreshPlan::Update { .. })
    ));
    assert_eq!(grant_plan(&plan).layer_refresh_uid, None);
    let changes = manager.commit(&HpManager::default(), plan);

    assert_eq!(changes.change.refreshed[0].before.layer, Some(6));
    assert_eq!(changes.change.refreshed[0].after.layer, Some(12));
    assert!(!changes.refresh_wire[0].echo_before);
}

#[test]
fn halo_fanout_targets_and_child_uids_are_planned() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    let mut hp = HpManager::default();
    manager.seed(&fight);
    hp.seed(&fight);
    let plan = manager
        .plan(
            &hp,
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id: 30860153,
                amount: Some(2),
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();

    let grant = grant_plan(&plan);
    assert_eq!(grant.uid.unwrap().uid, 2);
    assert_eq!(
        grant
            .fanout
            .iter()
            .map(|fanout| (fanout.spec.route.target_uid, fanout.uid.uid))
            .collect::<Vec<_>>(),
        vec![(11, 3)]
    );

    let changes = manager.commit(&hp, plan);
    let added = changes.change.added.as_ref().unwrap();
    assert!(added.fanout.is_empty());
    assert_eq!(changes.fanout.len(), 1);
    assert_eq!(
        changes.fanout[0].rule,
        DefinitionKey::new(771, "MasterHalo")
    );
    assert_eq!(changes.fanout[0].added[0].buff.uid, Some(3));
    assert_eq!(changes.events().len(), 2);
}

#[test]
fn base_halo_regrant_refreshes_the_same_team_instances() {
    crate::test_support::init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: [-1, -2, -3]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    team_type: Some(2),
                    current_hp: Some(100),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    let mut hp = HpManager::default();
    manager.seed(&fight);
    hp.seed(&fight);
    let grant = || {
        BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: -1,
            target_uid: -1,
            buff_id: 109320111,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };

    let initial = manager.execute(&hp, grant()).unwrap();
    let root = initial.change.added.as_ref().unwrap();
    let mut instances = vec![(root.target_uid, root.buff.uid.unwrap())];
    instances.extend(initial.fanout[0].added.iter().map(|added| {
        assert!(added.markers.iter().all(|marker| marker.effect_type
            != sonettobuf::effect_type_enum::EffectType::Halobase as i32));
        (added.target_uid, added.buff.uid.unwrap())
    }));
    assert_eq!(
        instances.iter().map(|(uid, _)| *uid).collect::<Vec<_>>(),
        vec![-1, -2, -3]
    );
    assert_eq!(
        root.markers
            .iter()
            .filter(|marker| {
                marker.effect_type == sonettobuf::effect_type_enum::EffectType::Halobase as i32
            })
            .count(),
        1
    );

    let refreshed = manager.execute(&hp, grant()).unwrap();
    let mut refreshed_instances = refreshed
        .change
        .refreshed
        .iter()
        .map(|update| (update.target_uid, update.after.uid.unwrap()))
        .collect::<Vec<_>>();
    refreshed_instances.extend(refreshed.fanout[0].refreshed.iter().map(|refresh| {
        assert!(refresh.markers.iter().all(|marker| marker.effect_type
            != sonettobuf::effect_type_enum::EffectType::Halobase as i32));
        (refresh.update.target_uid, refresh.update.after.uid.unwrap())
    }));
    assert_eq!(refreshed_instances, instances);
    assert_eq!(
        refreshed.refresh_wire[0]
            .markers
            .iter()
            .filter(|marker| {
                marker.effect_type == sonettobuf::effect_type_enum::EffectType::Halobase as i32
            })
            .map(|marker| (marker.target_uid, marker.effect_type))
            .collect::<Vec<_>>(),
        vec![(
            -1,
            sonettobuf::effect_type_enum::EffectType::Halobase as i32,
        )]
    );
}

#[test]
fn opposing_team_halo_fanout_uses_the_configured_scope() {
    crate::test_support::init_config();
    let entities = |team_type, uids: &[i64]| FightTeam {
        entitys: uids
            .iter()
            .map(|uid| FightEntityInfo {
                uid: Some(*uid),
                team_type: Some(team_type),
                current_hp: Some(100),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(entities(1, &[10, 11])),
        defender: Some(entities(2, &[-1, -2, -3])),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    let mut hp = HpManager::default();
    manager.seed(&fight);
    hp.seed(&fight);

    let result = manager
        .execute(
            &hp,
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id: 31390163,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();

    assert_eq!(
        result.fanout[0]
            .added
            .iter()
            .map(|added| added.target_uid)
            .collect::<Vec<_>>(),
        vec![-1, -2, -3]
    );
}

#[test]
fn layered_master_halo_refreshes_existing_allied_copies() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: [10, 11, 12]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    let mut hp = HpManager::default();
    manager.seed(&fight);
    hp.seed(&fight);
    let grant = |amount| {
        BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: 10,
            target_uid: 10,
            buff_id: 30950113,
            amount: Some(amount),
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };

    let initial = manager.execute(&hp, grant(8)).unwrap();
    let child_uids = initial.fanout[0]
        .added
        .iter()
        .map(|added| added.buff.uid.unwrap())
        .collect::<Vec<_>>();
    let refreshed = manager.execute(&hp, grant(4)).unwrap();

    assert_eq!(refreshed.change.refreshed[0].before.layer, Some(8));
    assert_eq!(refreshed.change.refreshed[0].after.layer, Some(12));
    assert_eq!(refreshed.fanout.len(), 1);
    assert_eq!(
        refreshed.fanout[0].rule,
        DefinitionKey::new(822, "LayerMasterHalo")
    );
    assert!(refreshed.fanout[0].added.is_empty());
    assert_eq!(
        refreshed.fanout[0]
            .refreshed
            .iter()
            .map(|change| (
                change.update.after.uid.unwrap(),
                change.update.before.layer.unwrap(),
                change.update.after.layer.unwrap(),
                change
                    .markers
                    .iter()
                    .map(|marker| marker.effect_type)
                    .collect::<Vec<_>>(),
            ))
            .collect::<Vec<_>>(),
        child_uids
            .into_iter()
            .map(|uid| {
                (
                    uid,
                    8,
                    12,
                    vec![
                        sonettobuf::effect_type_enum::EffectType::Layerslavehalo as i32,
                        sonettobuf::effect_type_enum::EffectType::Attr as i32,
                    ],
                )
            })
            .collect::<Vec<_>>()
    );
    assert_eq!(refreshed.events().len(), 3);
}

#[test]
fn identical_indefinite_halo_grant_keeps_master_and_children() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    let mut hp = HpManager::default();
    manager.seed(&fight);
    hp.seed(&fight);
    let grant = || {
        BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: 10,
            target_uid: 10,
            buff_id: 31050141,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };

    manager.execute(&hp, grant()).unwrap();
    assert!(manager.has_buff_id(10, 31050141));
    assert!(manager.has_buff_id(10, 31050144));
    assert!(manager.has_buff_id(11, 31050144));

    let plan = manager.plan(&hp, grant()).unwrap();
    assert_eq!(grant_plan(&plan).action, GrantAction::KeepExisting);
    let unchanged = manager.commit(&hp, plan);
    assert!(unchanged.change.added.is_none());
    assert!(unchanged.change.removed.is_empty());
    assert!(manager.has_buff_id(10, 31050141));
    assert!(manager.has_buff_id(10, 31050144));
    assert!(manager.has_buff_id(11, 31050144));
}

#[test]
fn identical_hidden_permanent_marker_grant_keeps_its_uid() {
    crate::test_support::init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    let mut hp = HpManager::default();
    manager.seed(&fight);
    hp.seed(&fight);
    let grant = || {
        BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: -1,
            target_uid: -1,
            buff_id: 530000789,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };

    let first = manager.execute(&hp, grant()).unwrap();
    let uid = first.change.added.unwrap().buff.uid;
    let plan = manager.plan(&hp, grant()).unwrap();
    assert_eq!(grant_plan(&plan).action, GrantAction::KeepExisting);

    let unchanged = manager.commit(&hp, plan);
    assert!(unchanged.change.added.is_none());
    assert!(unchanged.change.removed.is_empty());
    assert!(unchanged.change.refreshed.is_empty());
    assert_eq!(manager.active_for(-1).next().unwrap().uid, uid);
}
