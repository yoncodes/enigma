use super::*;

#[test]
fn defender_type_998_self_buffs_use_enter_fight_uid_lane() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
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
        .add(&HpManager::default(), -1, -1, 70015, 0)
        .unwrap();
    let next = manager
        .add(&HpManager::default(), -1, -1, 530000111, 3)
        .unwrap();

    assert_eq!(added.buff.uid, Some(100002));
    assert_eq!(next.buff.uid, Some(100003));
}

#[test]
fn permanent_battle_rule_attribute_counts_once_at_layer_zero() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    manager.add_replacing_excluded_with_layer_specified(
        &HpManager::default(),
        -1,
        -1,
        70015,
        0,
        true,
    );
    manager.add_replacing_excluded_with_layer_specified(
        &HpManager::default(),
        -1,
        -1,
        70015,
        0,
        true,
    );

    assert_eq!(manager.attribute_delta(-1, AttrId::Attack), 100);
    assert_eq!(manager.attribute_delta(-1, AttrId::DmgTakenReduction), 150);
}

#[test]
fn defender_regular_child_buffs_use_global_defender_sequence() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    team_type: Some(2),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    team_type: Some(2),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    let first = manager
        .add(&HpManager::default(), -1, -1, 530000111, 3)
        .unwrap();
    let second = manager
        .add(&HpManager::default(), -2, -2, 530000111, 3)
        .unwrap();

    assert_eq!(first.buff.uid, Some(100001));
    assert_eq!(second.buff.uid, Some(100002));
}

#[test]
fn linked_add_buff_to_enter_uses_defender_enter_uid_lane() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    buff_id: Some(31280119),
                    uid: Some(2),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
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
        .add(&HpManager::default(), 10, -1, 31280111, 0)
        .unwrap();

    assert_eq!(added.buff.uid, Some(100002));
}

#[test]
fn static_power_max_adds_parse_power_buff_features() {
    init_config();
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

    manager
        .add(&HpManager::default(), 10, 10, 31050147, 0)
        .unwrap();

    assert_eq!(
        manager.static_power_max_adds(),
        vec![BuffPowerMaxAdd {
            buff_uid: 2,
            owner_uid: 10,
            power_id: 1,
            delta: 2,
        }]
    );
}

#[test]
fn static_hp_max_add_rates_parse_hp_buff_features() {
    init_config();
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

    manager
        .add(&HpManager::default(), 10, 10, 30800151, 0)
        .unwrap();

    assert_eq!(
        manager.static_hp_max_add_rates(),
        vec![BuffHpMaxAddRate {
            buff_uid: 2,
            owner_uid: 10,
            permille: 50,
        }]
    );
}

#[test]
fn active_buff_features_can_link_passive_skill_effects() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    buff_id: Some(434435),
                    uid: Some(2),
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

    let links = manager.passive_skill_links_for(10);

    assert_eq!(links.len(), 1);
    assert_eq!(links[0].owner_uid, 10);
    assert_eq!(links[0].runtime_target_uid, 10);
    assert_eq!(links[0].skill_id, 434425);
}

#[test]
fn passive_skill_link_activates_at_the_configured_buff_layer() {
    init_config();
    let manager = |layer| {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        buff_id: Some(12110011),
                        uid: Some(2),
                        from_uid: Some(-1),
                        layer: Some(layer),
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
    };

    assert!(manager(9).passive_skill_links_for(10).is_empty());
    assert_eq!(
        manager(10).passive_skill_links_for(10)[0].skill_id,
        12110011
    );
}

#[test]
fn settlement_advances_only_matching_take_stage_durations() {
    init_config();
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
    manager.seed(&fight);
    let added = manager
        .add(&HpManager::default(), -1, -1, 530000414, 0)
        .unwrap();

    let updates = manager.advance_durations(
        crate::engine::skill::buff_act::effect_time::ROUND_END_ENTITY_SETTLEMENT,
    );

    assert_eq!(added.buff.duration, Some(2));
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].refreshed[0].after.duration, Some(1));
}

#[test]
fn duration_updates_follow_requested_owner_order() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);
    let mut hp = HpManager::default();
    hp.seed(&fight);
    let second = manager.add(&hp, 20, 20, 530000414, 0).unwrap();
    manager.add(&hp, 10, 10, 530000414, 0).unwrap();

    let plans = manager.plan_duration_advances(
        crate::engine::skill::buff_act::effect_time::ROUND_END_ENTITY_SETTLEMENT,
        &[10, 20],
    );
    let first = manager.commit_duration_advance(plans[0]).unwrap();

    assert_eq!(first.refreshed[0].target_uid, 10);
    assert_eq!(
        manager
            .snapshot(20, second.buff.uid.unwrap())
            .unwrap()
            .duration,
        Some(2)
    );

    let updates = std::iter::once(first)
        .chain(
            plans[1..]
                .iter()
                .filter_map(|plan| manager.commit_duration_advance(*plan)),
        )
        .collect::<Vec<_>>();

    assert_eq!(
        updates
            .iter()
            .map(|change| change.refreshed[0].target_uid)
            .collect::<Vec<_>>(),
        vec![10, 20]
    );
}

#[test]
fn duration_snapshot_does_not_advance_buffs_added_during_the_event() {
    init_config();
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
    let mut hp = HpManager::default();
    hp.seed(&fight);
    let existing = manager.add(&hp, 10, 10, 31170001, 0).unwrap();
    let snapshot = vec![existing.buff.uid.unwrap()];
    let added_during_event = manager.add(&hp, 10, 10, 31080143, 0).unwrap();

    let updates = manager.advance_durations_for_snapshot(
        crate::engine::skill::buff_act::effect_time::ROUND_START_DURATION,
        &[10],
        &snapshot,
    );

    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].removed[0].buff.uid, existing.buff.uid);
    assert_eq!(
        manager
            .active_for(10)
            .find(|buff| buff.uid == added_during_event.buff.uid)
            .and_then(|buff| buff.duration),
        Some(1)
    );
}

#[test]
fn empty_duration_snapshot_does_not_advance_new_buffs() {
    init_config();
    let mut manager = BuffManager::default();
    let hp = HpManager::default();
    let added = manager.add(&hp, 10, 10, 31170001, 0).unwrap();

    let updates = manager.advance_durations_for_snapshot(
        crate::engine::skill::buff_act::effect_time::ROUND_START_DURATION,
        &[10],
        &[],
    );

    assert!(updates.is_empty());
    assert_eq!(
        manager
            .active_for(10)
            .find(|buff| buff.uid == added.buff.uid)
            .and_then(|buff| buff.duration),
        Some(1)
    );
}

#[test]
fn duration_settlement_removes_elapsed_independent_instances() {
    init_config();
    let mut manager = BuffManager::default();
    let hp = HpManager::default();
    manager.add(&hp, 3108, 3108, 31080145, 1).unwrap();

    let changes = manager.advance_durations(
        crate::engine::skill::buff_act::effect_time::ROUND_END_ENTITY_SETTLEMENT,
    );

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].removed[0].buff.buff_id, Some(31080145));
    assert_eq!(manager.active_for(3108).count(), 0);
}

#[test]
fn setting_duration_updates_every_matching_buff_instance() {
    init_config();
    let mut manager = BuffManager::default();
    let hp = HpManager::default();
    manager.add(&hp, 10, 10, 530000414, 0).unwrap();
    manager.add(&hp, 10, 10, 530000414, 0).unwrap();

    let change = manager.set_duration_by_id(10, 530000414, 5);

    assert_eq!(change.refreshed.len(), 2);
    assert!(manager.active_for(10).all(|buff| buff.duration == Some(5)));
}

#[test]
fn configured_stack_threshold_replaces_the_carrier_buff() {
    init_config();
    let mut manager = BuffManager::default();
    let hp = HpManager::default();

    for _ in 0..8 {
        manager.add_replacing_excluded(&hp, 10, 10, 2295013, 1);
    }

    assert!(!manager.has_buff_id(10, 2295013));
    assert!(manager.has_buff_id(10, 2295023));
}
