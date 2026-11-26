use super::*;

#[test]
fn update_and_delete_keep_active_state() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    uid: Some(2),
                    buff_id: Some(101),
                    layer: Some(1),
                    count: Some(1),
                    duration: Some(2),
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

    let update = manager.update(10, 2, Some(3), Some(4), Some(5)).unwrap();
    assert_eq!(update.before.layer, Some(1));
    assert_eq!(update.after.layer, Some(3));
    assert_eq!(manager.active_for(10).next().unwrap().count, Some(4));

    let removed = manager.delete(10, 2).pop().unwrap();
    assert_eq!(removed.buff.uid, Some(2));
    assert_eq!(manager.active_for(10).count(), 0);
}

#[test]
fn removes_buffs_by_enum_status() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                buffs: vec![
                    BuffInfo {
                        uid: Some(100001),
                        buff_id: Some(530000111),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(100002),
                        buff_id: Some(530000112),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    let removed = manager.remove_by_statuses(-1, &[BuffStatus::PositiveStatus], 0, 30003);

    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].buff.buff_id, Some(530000111));
    assert_eq!(removed[0].config_effect, 30003);
    assert!(manager.has_buff_id(-1, 530000112));
}

#[test]
fn add_replacing_excluded_removes_configured_resident_buffs() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                buffs: vec![BuffInfo {
                    uid: Some(100001),
                    buff_id: Some(530000111),
                    from_uid: Some(-1),
                    count: Some(1),
                    layer: Some(3),
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

    let result = manager.add_replacing_excluded(&HpManager::default(), -1, -1, 530000417, 0);

    assert_eq!(result.removed.len(), 1);
    assert_eq!(result.removed[0].buff.buff_id, Some(530000111));
    assert_eq!(result.removed[0].buff.uid, Some(100001));
    assert_eq!(result.added.unwrap().buff.buff_id, Some(530000417));
    assert!(!manager.has_buff_id(-1, 530000111));
    assert!(manager.has_buff_id(-1, 530000417));
}

#[test]
fn resident_exclusion_rejects_the_forbidden_buff_without_mutating_state() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                buffs: vec![BuffInfo {
                    uid: Some(100001),
                    buff_id: Some(530000417),
                    from_uid: Some(-1),
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

    let result = manager.add_replacing_excluded(&HpManager::default(), -1, -1, 530000112, 0);

    let rejected = result.rejected.unwrap();
    assert_eq!(rejected.blocker_buff_id, 530000417);
    assert_eq!(rejected.buff.buff_id, Some(530000112));
    assert!(manager.has_buff_id(-1, 530000417));
    assert!(!manager.has_buff_id(-1, 530000112));
}

#[test]
fn resident_status_exclusion_rejects_a_matching_category() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                buffs: vec![BuffInfo {
                    uid: Some(2),
                    buff_id: Some(2112021),
                    from_uid: Some(-1),
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

    let result = manager.add_replacing_excluded(&HpManager::default(), 10, -1, 30650204, 1);

    let rejected = result.rejected.unwrap();
    assert_eq!(rejected.blocker_buff_id, 2112021);
    assert_eq!(rejected.buff.buff_id, Some(30650204));
    assert_eq!(rejected.buff.layer, Some(0));
    assert!(manager.has_buff_id(-1, 2112021));
    assert!(!manager.has_buff_id(-1, 30650204));
}

#[test]
fn a_refusing_buff_type_rejects_its_described_types_only_while_held() {
    init_config();
    let holder = |uid, marker| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(1),
        buffs: vec![BuffInfo {
            uid: Some(uid * 10),
            buff_id: Some(marker),
            from_uid: Some(uid),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            // J's ultimate marker (type 30940191), Refuse Absolute Defense (type 8092),
            // Refuse Chorus (type 8304).
            entitys: vec![
                holder(10, 30940191),
                holder(11, 22341),
                holder(12, 30485),
                holder(13, 31210132),
            ],
            ..Default::default()
        }),
        ..Default::default()
    });
    let hp = HpManager::default();

    let rejected = manager
        .add_replacing_excluded(&hp, 10, 10, 4150001, 1)
        .rejected
        .unwrap();
    assert_eq!(rejected.blocker_buff_id, 30940191);
    assert!(!manager.has_buff_id(10, 4150001));

    // Both defense types, including another skill's defense.
    for defense in [3113, 3123] {
        let rejected = manager
            .add_replacing_excluded(&hp, 12, 11, defense, 0)
            .rejected
            .unwrap();
        assert_eq!(rejected.blocker_buff_id, 22341);
    }
    let rejected = manager
        .add_replacing_excluded(&hp, 12, 12, 30483, 1)
        .rejected
        .unwrap();
    assert_eq!(rejected.blocker_buff_id, 30485);

    let rejected = manager
        .add_replacing_excluded(&hp, 13, 13, 31210131, 1)
        .rejected
        .unwrap();
    assert_eq!(rejected.blocker_buff_id, 31210132);
    assert!(!manager.has_buff_id(13, 31210131));

    // Types the rule does not name still land.
    assert!(
        manager
            .add_replacing_excluded(&hp, 11, 11, 205, 0)
            .rejected
            .is_none()
    );

    // Once the attack's marker is removed, Burn lands again.
    manager.delete(10, 100);
    assert!(
        manager
            .add_replacing_excluded(&hp, 10, 10, 4150001, 1)
            .rejected
            .is_none()
    );
    assert!(manager.has_buff_id(10, 4150001));
}

#[test]
fn incoming_buff_wins_mutual_exclusion() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(2),
                    buff_id: Some(530000111),
                    from_uid: Some(-1),
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

    let result = manager.add_replacing_excluded(&HpManager::default(), -1, 10, 530000112, 0);

    assert!(result.rejected.is_none());
    assert_eq!(result.removed.len(), 1);
    assert_eq!(result.removed[0].buff.buff_id, Some(530000111));
    assert_eq!(result.added.unwrap().buff.buff_id, Some(530000112));
}

#[test]
fn buff_add_emits_static_feature_markers() {
    init_config();
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

    let attr = manager.add(&HpManager::default(), 10, 10, 209, 0).unwrap();
    let injury = manager
        .add(&HpManager::default(), 10, 10, 433011, 0)
        .unwrap();
    let overflow = manager
        .add(&HpManager::default(), 10, 10, 31250161, 0)
        .unwrap();
    let revive = manager
        .add(&HpManager::default(), 10, 10, 31250181, 0)
        .unwrap();

    assert_eq!(
        attr.markers[0].effect_type,
        sonettobuf::effect_type_enum::EffectType::Attr as i32
    );
    assert_eq!(
        injury.markers[0].effect_type,
        sonettobuf::effect_type_enum::EffectType::Teammateinjurycount as i32
    );
    assert_eq!(
        overflow.markers[0].effect_type,
        sonettobuf::effect_type_enum::EffectType::Expointoverflowbank as i32
    );
    assert_eq!(
        revive.markers[0].effect_type,
        sonettobuf::effect_type_enum::EffectType::Cure as i32
    );
}

#[test]
fn buff_owner_records_exact_pre_add_wire_effects() {
    init_config();
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

    let added = manager
        .add(&HpManager::default(), 10, 10, 7_280_002, 0)
        .unwrap();

    assert_eq!(
        added.pre_effects,
        vec![BuffWireEffectResult {
            target_uid: 10,
            effect_type: sonettobuf::effect_type_enum::EffectType::Nuodikarandomattacknum as i32,
            effect_num: 0,
            effect_num1: 1,
        }]
    );
}

#[test]
fn explicit_positive_layer_uses_child_uid_lane() {
    init_config();
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

    let first = manager.add(&HpManager::default(), 10, 10, 101, 0).unwrap();
    let layered = manager
        .add(&HpManager::default(), 10, 10, 530000111, 3)
        .unwrap();
    let next = manager.add(&HpManager::default(), 10, 10, 101, 0).unwrap();

    assert_eq!(first.buff.uid, Some(2));
    assert_eq!(layered.buff.uid, Some(3));
    assert_eq!(next.buff.uid, Some(5));
}

#[test]
fn defaulted_stack_layer_uses_child_uid_lane() {
    init_config();
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

    let first = manager
        .add(&HpManager::default(), 10, 10, 31140142, 0)
        .unwrap();
    let stacked = manager.add_replacing_excluded_with_layer_specified(
        &HpManager::default(),
        10,
        10,
        31140143,
        0,
        false,
    );

    assert_eq!(first.buff.uid, Some(2));
    assert_eq!(stacked.added.unwrap().buff.uid, Some(3));
}

#[test]
fn resident_exclusion_rejects_a_buff_of_the_excluded_type() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    uid: Some(100001),
                    buff_id: Some(2295023),
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

    // Master Assassin (type 229502) excludes "2#229501"; Apprentice Assassin 2295013 is of
    // type 229501.
    let result = manager.add_replacing_excluded(&HpManager::default(), 10, 10, 2295013, 0);

    assert_eq!(result.rejected.unwrap().blocker_buff_id, 2295023);
    assert!(!manager.has_buff_id(10, 2295013));
}
