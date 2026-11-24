use super::*;

#[test]
fn attribute_features_use_stack_layers_before_trigger_count() {
    assert_eq!(
        attribute_amount(
            &BuffInfo {
                layer: Some(3),
                count: Some(1),
                ..Default::default()
            },
            None
        ),
        3
    );
}

#[test]
fn broad_buff_status_count_uses_skill_bufftype_category() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![
                    BuffInfo {
                        buff_id: Some(90071),
                        uid: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(30860113),
                        uid: Some(2),
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

    assert_eq!(manager.buff_status_count(10, &[8]), 2);
}

#[test]
fn broad_buff_status_count_uses_configured_include_type_membership() {
    init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    buff_id: Some(31340007),
                    uid: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    assert_eq!(manager.buff_status_count(10, &[1]), 1);
    assert_eq!(manager.buff_status_count(10, &[4]), 1);
    assert_eq!(manager.buff_status_count(10, &[2]), 0);
}

#[test]
fn visible_status_type_count_excludes_hidden_buffs_and_duplicate_types() {
    init_config();
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![
                    BuffInfo {
                        buff_id: Some(400104),
                        uid: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(400105),
                        uid: Some(2),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(31440116),
                        uid: Some(3),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(31440114),
                        uid: Some(4),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    assert_eq!(manager.visible_buff_status_type_count(10, &[1, 5]), 2);
}

#[test]
fn visible_stacked_status_count_uses_direct_categories_and_type_ten_layers() {
    init_config();
    let buffs = |ids: &[i32], stacked_layer, uid_offset| {
        ids.iter()
            .enumerate()
            .map(|(index, buff_id)| BuffInfo {
                buff_id: Some(*buff_id),
                uid: Some(index as i64 + uid_offset),
                layer: (*buff_id == 103374114).then_some(stacked_layer),
                ..Default::default()
            })
            .collect()
    };
    let mut manager = BuffManager::default();
    manager.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    buffs: buffs(&[31450171, 30250001, 31440116, 31450111, 31450143], 0, 1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    buffs: buffs(&[31450171, 31440116, 31450111, 31450143, 103374114], 2, 101),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    });

    assert_eq!(manager.visible_stacked_status_type_count(10, &[1, 5]), 4);
    assert_eq!(manager.visible_stacked_status_type_count(11, &[1, 5]), 5);
}

#[test]
fn special_count_is_stored_on_the_marker_buff() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(777),
                    act_common_params: Some("1003#2".into()),
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

    let changes = manager.add_special_count(10, &[777], 3);

    assert_eq!(changes.len(), 1);
    assert_eq!(manager.special_count(10, &[777]), 5);
    assert_eq!(
        changes[0].after.act_common_params.as_deref(),
        Some("1003#5")
    );
}

#[test]
fn active_features_fall_back_to_stack_layer_when_count_is_zero() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(31200142),
                    count: Some(0),
                    layer: Some(2),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);

    assert_eq!(managers.buff.active_features(&managers.hp)[0].amount, 2);
}

#[test]
fn materialized_sub_buff_owns_its_features_once() {
    crate::test_support::init_config();
    let managers = BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                buffs: vec![
                    BuffInfo {
                        uid: Some(10),
                        buff_id: Some(31260151),
                        from_uid: Some(2),
                        count: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(11),
                        buff_id: Some(31260201),
                        from_uid: Some(2),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    let features = managers.buff.active_features(&managers.hp);
    assert_eq!(
        features
            .iter()
            .filter(|feature| feature.buff_id == 31260201 && feature.act_id() == Some(932))
            .map(|feature| feature.buff_uid)
            .collect::<Vec<_>>(),
        [11]
    );
}

#[test]
fn feature_liveness_reads_hp_manager() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    buff_id: Some(205),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut buffs = BuffManager::default();
    let mut hp = HpManager::default();
    buffs.seed(&fight);
    hp.seed(&fight);

    assert!(buffs.active_features(&hp)[0].owner_alive);
    hp.lose(10, 100, 0).unwrap();
    assert!(!buffs.active_features(&hp)[0].owner_alive);
}
