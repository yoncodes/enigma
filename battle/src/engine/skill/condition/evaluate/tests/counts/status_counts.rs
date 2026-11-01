use super::*;

#[test]
fn per_buff_id_count_repeats_once_per_matching_layer() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    for _ in 0..5 {
        managers.buff.add(&managers.hp, 10, 10, 31260151, 1);
    }
    let condition = ParsedCondition {
        opcode: 61302,
        type_name: "PerBuffIdCount".into(),
        kind: ParsedConditionKind::BuffIdCount {
            buff_ids: vec![31260151],
            compare: ConditionCompare::GreaterThanOrEqual,
            threshold: 1,
        },
        raw_args: vec!["31260151".into()],
    };

    assert_eq!(
        conditions_fire_count(
            &[condition],
            10,
            &[10],
            Some(&managers),
            &TargetPool::from_fight(&fight),
            TargetContext::default(),
        ),
        5
    );
}

#[test]
fn per_buff_id_counts_a_matching_non_first_id() {
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
    let mut managers = BattleManagers::seeded(&fight);
    for _ in 0..2 {
        managers.buff.add(&managers.hp, 10, 10, 31260151, 1);
    }
    let condition = ParsedCondition {
        opcode: 59203,
        type_name: "PerBuffId".into(),
        kind: ParsedConditionKind::BuffIdCount {
            buff_ids: vec![109320110, 31260151],
            compare: ConditionCompare::GreaterThanOrEqual,
            threshold: 1,
        },
        raw_args: vec!["109320110".into(), "31260151".into()],
    };

    assert_eq!(
        conditions_fire_count(
            &[condition],
            10,
            &[10],
            Some(&managers),
            &TargetPool::from_fight(&fight),
            TargetContext::default(),
        ),
        2
    );
}

#[test]
fn team_status_type_count_repeats_per_distinct_buff_type() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    buffs: vec![
                        BuffInfo {
                            buff_id: Some(400901),
                            uid: Some(1),
                            ..Default::default()
                        },
                        BuffInfo {
                            buff_id: Some(400902),
                            uid: Some(2),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    buffs: vec![
                        BuffInfo {
                            buff_id: Some(31340007),
                            uid: Some(3),
                            ..Default::default()
                        },
                        BuffInfo {
                            buff_id: Some(712313),
                            uid: Some(4),
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
    let managers = BattleManagers::seeded(&fight);
    let condition = ParsedCondition {
        opcode: 539301,
        type_name: "PerSelfTeamTypeType2BuffTypeIdNum".into(),
        kind: ParsedConditionKind::PerTeamBuffStatusTypeCount {
            status_ids: vec![1, 3, 5, 7, 14],
            divisor: 3,
            max_count: 5,
        },
        raw_args: vec!["3".into(), "5".into(), "1,3,5,7,14".into()],
    };

    assert_eq!(
        conditions_fire_count(
            &[condition],
            10,
            &[10, 11],
            Some(&managers),
            &TargetPool::from_fight(&fight),
            TargetContext::default(),
        ),
        1
    );
}

#[test]
fn target_status_type_count_repeats_once_per_distinct_type() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![
                    BuffInfo {
                        buff_id: Some(400901),
                        uid: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(400902),
                        uid: Some(2),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(712313),
                        uid: Some(3),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let condition = ParsedCondition {
        opcode: 85203,
        type_name: "PerBuffTypeCountGroupByTypeId".into(),
        kind: ParsedConditionKind::PerTeamBuffStatusTypeCount {
            status_ids: vec![1, 3, 5, 14],
            divisor: 1,
            max_count: i32::MAX,
        },
        raw_args: vec!["1,3,5".into(), "14".into()],
    };

    assert_eq!(
        conditions_fire_count(
            &[condition],
            10,
            &[10],
            Some(&managers),
            &TargetPool::from_fight(&fight),
            TargetContext::default(),
        ),
        2
    );
}

#[test]
fn visible_status_thresholds_evaluate_targets_without_team_aggregation() {
    init_config();
    let buffs = |ids: &[i32], uid_offset| {
        ids.iter()
            .enumerate()
            .map(|(index, buff_id)| BuffInfo {
                buff_id: Some(*buff_id),
                uid: Some(index as i64 + uid_offset),
                from_uid: Some(10),
                ..Default::default()
            })
            .collect()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    buffs: buffs(
                        &[
                            31450171, 31450141, 31450143, 30250001, 31440116, 31450111, 31450132,
                        ],
                        1,
                    ),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(100),
                    buffs: buffs(
                        &[
                            31450171, 30250001, 31440116, 31450111, 31450132, 31440113, 30250003,
                        ],
                        20,
                    ),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);

    assert_eq!(managers.buff.visible_buff_status_type_count(10, &[1, 5]), 5);
    assert_eq!(managers.buff.visible_buff_status_type_count(11, &[1, 5]), 7);
    assert!(conditions_match(
        &[exact_condition(43203, "HasTypeBuffLessThan", &["6", "1,5"],)],
        10,
        &[10, 11],
        Some(&managers),
        &TargetPool::from_fight(&fight),
        TargetContext::default(),
    ));
    assert!(!conditions_match(
        &[exact_condition(
            42203,
            "HasTypeBuffMoreThan",
            &["10", "1,5"],
        )],
        10,
        &[10, 11],
        Some(&managers),
        &TargetPool::from_fight(&fight),
        TargetContext::default(),
    ));
}

#[test]
fn bullet_count_repeats_per_distinct_effective_type() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![
                    BuffInfo {
                        buff_id: Some(722501),
                        uid: Some(1),
                        count: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(722502),
                        uid: Some(2),
                        count: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(31020111),
                        uid: Some(3),
                        count: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(31020112),
                        uid: Some(4),
                        count: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(31020113),
                        uid: Some(5),
                        count: Some(0),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(400901),
                        uid: Some(6),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let repeats = |divisor, max_count| {
        conditions_fire_count(
            &[ParsedCondition {
                opcode: 651203,
                type_name: "PerBullet".into(),
                kind: ParsedConditionKind::PerBullet { divisor, max_count },
                raw_args: vec![divisor.to_string(), max_count.to_string()],
            }],
            10,
            &[10],
            Some(&managers),
            &pool,
            TargetContext::default(),
        )
    };

    assert_eq!(repeats(1, 8), 3);
    assert_eq!(repeats(2, 8), 1);
    assert_eq!(repeats(4, 8), 0);
    assert_eq!(repeats(1, 2), 2);
}

#[test]
fn buff_group_count_repeats_once_per_layer_across_targets() {
    init_config();
    let poison = |uid, amount| BuffInfo {
        uid: Some(uid),
        buff_id: Some(30560101),
        from_uid: Some(10),
        count: Some(amount),
        layer: Some(amount),
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
                    buffs: vec![poison(1, 2)],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    buffs: vec![poison(2, 1)],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let condition = ParsedCondition {
        opcode: 669203,
        type_name: "PerBuffGroupCount".into(),
        kind: ParsedConditionKind::PerBuffGroupCount { group_id: 7 },
        raw_args: vec!["7".into()],
    };

    assert_eq!(
        conditions_fire_count(
            &[condition],
            10,
            &[-1, -2],
            Some(&managers),
            &TargetPool::from_fight(&fight),
            TargetContext::default(),
        ),
        3
    );
}

#[test]
fn accumulated_team_buff_count_preserves_all_crossed_thresholds() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.buff.add(&managers.hp, 10, 10, 90071, 30);
    let condition = ParsedCondition {
        opcode: 583004,
        type_name: "AccTeamAddBuffCountByBuffId".into(),
        kind: ParsedConditionKind::AccBuffAddedCount {
            buff_ids: vec![90071],
            threshold: 8,
            scope: BuffAddedScope::Team,
        },
        raw_args: vec!["90071".into(), "8".into()],
    };

    assert_eq!(
        conditions_fire_count(
            &[condition],
            10,
            &[10],
            Some(&managers),
            &TargetPool::from_fight(&fight),
            TargetContext {
                added_buff_id: 90071,
                added_buff_amount: 30,
                added_buff_target_uid: 10,
                ..Default::default()
            },
        ),
        3
    );
}

#[test]
fn round_power_conditions_preserve_consumed_and_overflow_counts() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                power_infos: vec![PowerInfo {
                    power_id: Some(1),
                    num: Some(4),
                    max: Some(5),
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.eureka.add(10, 10, 1, -3, 0);
    managers.eureka.add(10, 10, 1, 8, 0);
    let condition = |kind| ParsedCondition {
        opcode: 0,
        type_name: String::new(),
        kind,
        raw_args: Vec::new(),
    };

    for (kind, expected) in [
        (
            ParsedConditionKind::PowerConsumed {
                power_id: 1,
                max_count: 99,
            },
            3,
        ),
        (
            ParsedConditionKind::PowerOverflow {
                power_id: 1,
                max_count: 2,
            },
            2,
        ),
    ] {
        assert_eq!(
            conditions_fire_count(
                &[condition(kind)],
                10,
                &[10],
                Some(&managers),
                &TargetPool::from_fight(&fight),
                TargetContext::default(),
            ),
            expected
        );
    }
}

#[test]
fn power_ratio_reads_current_and_max_resource_values() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                power_infos: vec![PowerInfo {
                    power_id: Some(9),
                    num: Some(10),
                    max: Some(10),
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let condition = exact_condition(749301, "PowerRatio", &["9", "1", "1000"]);
    let matches = |managers: &BattleManagers| {
        conditions_match(
            std::slice::from_ref(&condition),
            10,
            &[10],
            Some(managers),
            &pool,
            TargetContext::default(),
        )
    };

    assert!(matches(&managers));
    managers.eureka.add(10, 10, 9, -1, 0);
    assert!(!matches(&managers));
}

#[test]
fn bound_pair_threshold_does_not_sum_layers_across_entities() {
    init_config();
    let matches = |layer| {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: [10, 11]
                    .into_iter()
                    .map(|uid| FightEntityInfo {
                        uid: Some(uid),
                        current_hp: Some(1),
                        buffs: vec![BuffInfo {
                            buff_id: Some(31000303),
                            uid: Some(uid),
                            duration: Some(1),
                            layer: Some(layer),
                            ..Default::default()
                        }],
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        conditions_match(
            &[exact_condition(
                535212,
                "TypeIdBuffCountMoreThan",
                &["31000303", "8"],
            )],
            10,
            &[10, 11],
            Some(&managers),
            &TargetPool::from_fight(&fight),
            TargetContext::default(),
        )
    };

    assert!(!matches(4));
    assert!(matches(8));
}

#[test]
fn poison_group_presence_is_evaluated_per_target() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        buff_id: Some(31040005),
                        uid: Some(400001),
                        duration: Some(2),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let has = ParsedCondition {
        opcode: 77208,
        type_name: "HasBuffGroup".into(),
        kind: ParsedConditionKind::BuffGroup(vec![7]),
        raw_args: vec!["7".into()],
    };
    let absent = ParsedCondition {
        opcode: 78208,
        type_name: "NoBuffGroup".into(),
        kind: ParsedConditionKind::NoBuffGroup(vec![7]),
        raw_args: vec!["7".into()],
    };

    assert!(conditions_match(
        std::slice::from_ref(&has),
        10,
        &[-1],
        Some(&managers),
        &pool,
        TargetContext::default(),
    ));
    assert!(conditions_match(
        std::slice::from_ref(&absent),
        10,
        &[-2],
        Some(&managers),
        &pool,
        TargetContext::default(),
    ));
    assert!(!conditions_match(
        &[absent],
        10,
        &[-1],
        Some(&managers),
        &pool,
        TargetContext::default(),
    ));
}


#[test]
fn per_aura_765203_repeats_once_per_current_eureka() {
    init_config();
    let fight = Fight { attacker: Some(FightTeam { entitys: vec![FightEntityInfo {
        uid: Some(10), current_hp: Some(100), power_infos: vec![PowerInfo {
            power_id: Some(1), num: Some(4), max: Some(10),
        }], ..Default::default() }], ..Default::default() }), ..Default::default() };
    let mut managers = BattleManagers::seeded(&fight);
    let condition = exact_condition(765203, "PerAura", &[]);
    let pool = TargetPool::from_fight(&fight);
    for count in [0, 1, 4, 10] {
        managers.eureka.set(10, 1, count);
        assert_eq!(conditions_fire_count(&[condition.clone()], 10, &[10], Some(&managers),
            &pool, TargetContext::default()), count);
    }
}
