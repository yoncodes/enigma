use super::*;

#[test]
fn child_grant_initializes_state_before_publishing_its_add() {
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
    let changes = manager
        .execute(
            &HpManager::default(),
            BuffCommand::GrantChild(BuffGrantChild {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(1004, "AddAttrBySpecialCount"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id: 70015,
                amount: Some(0),
                params: Some("1004#3".to_owned()),
                act_info: None,
            }),
        )
        .unwrap();
    let added = changes.change.added.as_ref().unwrap();

    assert_eq!(added.buff.act_common_params.as_deref(), Some("1004#3"));
    assert_eq!(
        manager
            .snapshot(10, added.buff.uid.unwrap())
            .unwrap()
            .act_common_params
            .as_deref(),
        Some("1004#3")
    );
    assert!(matches!(
        changes.events().as_slice(),
        [BattleEvent::BuffAdded(_)]
    ));
}

#[test]
fn triggered_child_allocation_comes_from_the_exact_buff_act_rule() {
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
    let changes = manager
        .execute(
            &HpManager::default(),
            BuffCommand::GrantChild(BuffGrantChild {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(1053, "AttrByHeatScale"),
                },
                source_uid: 0,
                target_uid: 10,
                buff_id: 31340007,
                amount: Some(0),
                params: None,
                act_info: Some(vec![BuffActInfo {
                    act_id: Some(1053),
                    param: vec![17],
                    str_param: Some(String::new()),
                }]),
            }),
        )
        .unwrap();
    let added = changes.change.added.as_ref().unwrap();

    assert_eq!(added.buff.uid, Some(3));
    assert_eq!(added.buff.act_info[0].param, vec![17]);
    assert_eq!(manager.snapshot(10, 3).unwrap().act_info[0].param, vec![17]);
    assert_eq!(
        manager
            .add(&HpManager::default(), 10, 10, 101, 0)
            .unwrap()
            .buff
            .uid,
        Some(5)
    );
}

#[test]
fn timed_child_renewal_expires_the_old_instance_before_the_stateful_grant() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1058),
                    buff_id: Some(31340007),
                    from_uid: Some(0),
                    duration: Some(1),
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
            BuffCommand::RenewTimedChild(BuffGrantChild {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(1053, "AttrByHeatScale"),
                },
                source_uid: 0,
                target_uid: 10,
                buff_id: 31340007,
                amount: Some(0),
                params: None,
                act_info: Some(vec![BuffActInfo {
                    act_id: Some(1053),
                    param: vec![60],
                    str_param: Some(String::new()),
                }]),
            }),
        )
        .unwrap();

    assert_eq!(changes.change.removed.len(), 1);
    assert_eq!(changes.change.removed[0].buff.uid, Some(1058));
    assert_eq!(changes.change.removed[0].buff.duration, Some(0));
    assert_eq!(changes.change.added.as_ref().unwrap().buff.uid, Some(1060));
    assert_eq!(
        changes.change.added.as_ref().unwrap().buff.act_info[0].param,
        [60]
    );
    assert!(changes.pre_add_markers_before_remove);
}

#[test]
fn repeated_grant_is_planned_by_the_manager() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
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
    let command = BuffCommand::Grant(BuffGrant {
        origin,
        source_uid: 10,
        target_uid: 10,
        buff_id: 31170006,
        amount: None,
        occurrences: 4,
        child_uid_reservations: 0,
    });
    let planned = manager.plan(&HpManager::default(), command).unwrap();
    let plan = grant_plan(&planned);

    assert_eq!(planned.origin, origin);
    assert_eq!(plan.uid.unwrap().uid, 4);

    let repeated = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin,
                source_uid: 10,
                target_uid: 10,
                buff_id: 31170006,
                amount: None,
                occurrences: 4,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();
    let following = manager
        .add(&HpManager::default(), 10, 10, 31080141, 0)
        .unwrap();

    assert_eq!(repeated.change.added.unwrap().buff.uid, Some(4));
    assert_eq!(following.buff.uid, Some(8));
}

#[test]
fn shared_uid_protocol_aggregates_repeated_layer_without_instance_reservations() {
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
    let repeated = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id: 31_170_006,
                amount: None,
                occurrences: 4,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();
    let following = manager.add(&HpManager::default(), 10, 10, 101, 0).unwrap();

    let added = repeated.change.added.unwrap().buff;
    assert_eq!(added.uid, Some(1002));
    assert_eq!(added.layer, Some(4));
    assert_eq!(following.buff.uid, Some(1004));
}

#[test]
fn shared_uid_layer_refresh_reserves_child_lane_before_the_next_grant() {
    crate::test_support::init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(1002),
                    buff_id: Some(31450121),
                    from_uid: Some(10),
                    layer: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let refreshed = manager
        .execute(
            &HpManager::default(),
            BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 10,
                target_uid: 10,
                buff_id: 31450121,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }),
        )
        .unwrap();
    let following = manager.add(&HpManager::default(), 10, 10, 101, 0).unwrap();

    assert_eq!(refreshed.change.refreshed[0].after.uid, Some(1002));
    assert_eq!(refreshed.change.refreshed[0].after.layer, Some(2));
    assert_eq!(following.buff.uid, Some(1005));
}
