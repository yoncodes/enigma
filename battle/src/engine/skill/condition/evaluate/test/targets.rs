use super::*;

#[test]
fn ultimate_level_matches_each_resolved_entity_snapshot() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                ex_skill_level: Some(4),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);

    assert!(condition_matches(
        &exact_condition(751104, "ExSkillLevel", &["4"]),
        10,
        &[10],
        None,
        &pool,
        TargetContext::default(),
    ));
    assert!(!condition_matches(
        &exact_condition(751104, "ExSkillLevel", &["3"]),
        10,
        &[10],
        None,
        &pool,
        TargetContext::default(),
    ));
}

#[test]
fn received_hit_afflatus_conditions_only_match_the_hit_owner() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    career: Some(1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    career: Some(3),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    career: Some(8),
                    weak_careers: vec![1],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    career: Some(8),
                    weak_careers: vec![1],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let matches = |condition: ParsedCondition, hit_source_uid, hit_target_uid| {
        conditions_match(
            &[condition],
            -1,
            &[-1],
            None,
            &pool,
            TargetContext {
                hit_source_uid,
                hit_target_uid,
                ..Default::default()
            },
        )
    };

    assert!(matches(
        exact_condition(33209, "HurtRestraint", &[]),
        10,
        -1
    ));
    assert!(!matches(
        exact_condition(33209, "HurtRestraint", &[]),
        10,
        -2
    ));
    assert!(matches(
        exact_condition(47209, "HurtNotRestraint", &[]),
        11,
        -1
    ));
    assert!(!matches(
        exact_condition(47209, "HurtNotRestraint", &[]),
        11,
        -2
    ));
}

#[test]
fn target_identity_reads_the_selected_skill_target() {
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
                    uid: Some(11),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let condition = ParsedCondition {
        opcode: 586208,
        type_name: "TargetIsTeamNoMe".into(),
        kind: ParsedConditionKind::TargetIdentity {
            mode: TargetIdentityMode::TargetIsAllyNotSelf,
            value: 0,
        },
        raw_args: Vec::new(),
    };

    assert!(conditions_match(
        &[condition],
        10,
        &[10],
        None,
        &TargetPool::from_fight(&fight),
        TargetContext {
            runtime_target_uid: 11,
            ..Default::default()
        },
    ));
}

#[test]
fn team_contains_hero_accepts_each_model_in_the_compound_assassination_gate() {
    init_config();
    let conditions = crate::engine::skill::condition::parse_conditions(
        config::configs::get(),
        "1000212#3122,3123&1001212",
    );
    let fight = |ally_model_id| Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3124),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    model_id: Some(ally_model_id),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let context = TargetContext {
        active_skill_assassinate: true,
        ..Default::default()
    };

    for ally_model_id in [3122, 3123] {
        assert!(conditions_match(
            &conditions,
            10,
            &[10],
            None,
            &TargetPool::from_fight(&fight(ally_model_id)),
            context,
        ));
    }
    assert!(!conditions_match(
        &conditions,
        10,
        &[10],
        None,
        &TargetPool::from_fight(&fight(3121)),
        context,
    ));
}

#[test]
fn per_target_career_condition_preserves_its_fire_count() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    career: Some(1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    career: Some(5),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(12),
                    career: Some(6),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let condition = ParsedCondition {
        opcode: 650002,
        type_name: "PerHasTargetCareerList".into(),
        kind: ParsedConditionKind::PerTargetCareerCount {
            careers: vec![5, 6],
            threshold: 2,
        },
        raw_args: Vec::new(),
    };

    assert_eq!(
        conditions_fire_count(
            &[condition],
            10,
            &[10],
            None,
            &TargetPool::from_fight(&fight),
            TargetContext::default(),
        ),
        2
    );
}

#[test]
fn team_career_threshold_counts_the_caster_once() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    career: Some(3),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    career: Some(3),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(12),
                    career: Some(3),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(13),
                    career: Some(5),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let condition = ParsedCondition {
        opcode: 562002,
        type_name: "CareerGroupHeroCountGE".into(),
        kind: ParsedConditionKind::TeamCareerCount {
            careers: vec![3],
            compare: ConditionCompare::GreaterThanOrEqual,
            threshold: 3,
        },
        raw_args: vec!["3".into(), "3".into()],
    };

    assert!(conditions_match(
        &[condition],
        10,
        &[10],
        None,
        &TargetPool::from_fight(&fight),
        TargetContext::default(),
    ));
}

#[test]
fn other_ally_damage_type_condition_repeats_up_to_its_cap() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: [3087, 3065, 3003, 3095]
                .into_iter()
                .enumerate()
                .map(|(index, model_id)| FightEntityInfo {
                    uid: Some(index as i64 + 10),
                    model_id: Some(model_id),
                    entity_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let condition = ParsedCondition {
        opcode: 573002,
        type_name: "PerTeamOtherEntityDmgType".into(),
        kind: ParsedConditionKind::OtherAllyDamageTypeCount {
            damage_type: crate::engine::skill::target::EntityDamageType::Mental,
            max_count: 2,
        },
        raw_args: vec!["2".into(), "2".into()],
    };

    assert_eq!(
        conditions_fire_count(
            &[condition],
            10,
            &[10],
            None,
            &pool,
            TargetContext::default(),
        ),
        2
    );
}

#[test]
fn natural_ally_condition_repeats_once_per_other_natural_ally() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    career: Some(5),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    career: Some(1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(12),
                    career: Some(3),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(13),
                    career: Some(6),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let condition = crate::engine::skill::condition::registry::parse(
        621002,
        "CareerNatureHeroNum",
        &["3".into()],
    )
    .unwrap();

    assert_eq!(
        conditions_fire_count(
            &[ParsedCondition {
                opcode: 621002,
                type_name: "CareerNatureHeroNum".into(),
                kind: condition,
                raw_args: vec!["3".into()],
            }],
            10,
            &[10],
            None,
            &TargetPool::from_fight(&fight),
            TargetContext::default(),
        ),
        2
    );
}

#[test]
fn team_model_presence_uses_the_casters_roster() {
    init_config();
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3123),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    model_id: Some(3124),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    });
    let absent = |model_id| ParsedCondition {
        opcode: 643004,
        type_name: "HasConditionTarget".into(),
        kind: ParsedConditionKind::TeamModelPresence {
            model_ids: vec![model_id],
            present: false,
        },
        raw_args: Vec::new(),
    };

    assert!(!condition_matches(
        &absent(3124),
        10,
        &[10],
        None,
        &pool,
        TargetContext::default(),
    ));
    assert!(condition_matches(
        &absent(3122),
        10,
        &[10],
        None,
        &pool,
        TargetContext::default(),
    ));
}

#[test]
fn added_magic_circle_matches_its_configured_id() {
    init_config();
    let pool = TargetPool::default();
    let condition = |ids| ParsedCondition {
        opcode: 711039,
        type_name: "AddMagicCircle".into(),
        kind: ParsedConditionKind::AddedMagicCircle(ids),
        raw_args: Vec::new(),
    };
    let context = TargetContext {
        added_magic_circle_id: 30001,
        ..Default::default()
    };

    assert!(!condition_matches(
        &condition(vec![30003]),
        10,
        &[10],
        None,
        &pool,
        context,
    ));
    assert!(condition_matches(
        &condition(vec![30001]),
        10,
        &[10],
        None,
        &pool,
        context,
    ));
    assert!(condition_matches(
        &condition(vec![0]),
        10,
        &[10],
        None,
        &pool,
        context,
    ));
}

#[test]
fn removed_magic_circle_only_matches_the_removed_field() {
    init_config();
    let pool = TargetPool::default();
    let condition = |ids| ParsedCondition {
        opcode: 712040,
        type_name: "RemoveMagicCircle".into(),
        kind: ParsedConditionKind::RemovedMagicCircle(ids),
        raw_args: Vec::new(),
    };
    let context = TargetContext {
        magic_circle_id: 30002,
        removed_magic_circle_id: 30003,
        ..Default::default()
    };

    assert!(condition_matches(
        &condition(vec![30003]),
        10,
        &[10],
        None,
        &pool,
        context,
    ));
    assert!(!condition_matches(
        &condition(vec![30002]),
        10,
        &[10],
        None,
        &pool,
        context,
    ));
}

#[test]
fn enemy_highest_buff_count_reads_the_enemy_team_maximum() {
    init_config();
    let enemy = |uid, layer| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(1),
        buffs: vec![BuffInfo {
            buff_id: Some(4150001),
            uid: Some(-uid),
            layer: Some(layer),
            ..Default::default()
        }],
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![enemy(-1, 5), enemy(-2, 6)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let matches = |threshold| {
        condition_matches(
            &ParsedCondition {
                opcode: 565104,
                type_name: "EnemyHighestTypeIdBuffCountMoreThan".into(),
                kind: ParsedConditionKind::EnemyHighestBuffTypeCount {
                    type_id: 4150001,
                    threshold,
                },
                raw_args: vec!["4150001".into(), threshold.to_string()],
            },
            10,
            &[10],
            Some(&managers),
            &pool,
            TargetContext::default(),
        )
    };

    assert!(matches(6));
    assert!(!matches(7));
}

#[test]
fn from_and_to_buff_checks_source_and_resolved_target_separately() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(229101),
                    duration: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(2),
                    buff_id: Some(229102),
                    duration: Some(2),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let condition = ParsedCondition {
        opcode: 1007204,
        type_name: "FromBuffAndToBuff".into(),
        kind: ParsedConditionKind::FromBuffAndToBuff {
            from_buff_id: 229101,
            to_buff_id: 229102,
        },
        raw_args: vec!["229101".into(), "229102".into()],
    };

    assert!(condition_matches(
        &condition,
        10,
        &[-1],
        Some(&managers),
        &TargetPool::from_fight(&fight),
        TargetContext::default(),
    ));
    assert!(!condition_matches(
        &condition,
        10,
        &[10],
        Some(&managers),
        &TargetPool::from_fight(&fight),
        TargetContext::default(),
    ));
}

#[test]
fn bound_ally_buff_types_follow_the_other_ally_action_source() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1),
                    buffs: vec![BuffInfo {
                        uid: Some(1),
                        buff_id: Some(31000201),
                        duration: Some(1),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(1),
                    buffs: vec![BuffInfo {
                        uid: Some(2),
                        buff_id: Some(31000171),
                        duration: Some(1),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(12),
                    current_hp: Some(1),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let conditions = [
        exact_condition(
            656212,
            "SelfBuffTypeTargetBuffTypes",
            &["31000201", "31000171,31000181"],
        ),
        exact_condition(403212, "SkillExtraType", &["1"]),
    ];
    let pool = TargetPool::from_fight(&fight);
    let matches = |active_skill_source_uid| {
        conditions_match(
            &conditions,
            10,
            &[10, 11, 12],
            Some(&managers),
            &pool,
            TargetContext {
                active_skill_source_uid,
                extra_skill_kind: 1,
                ..Default::default()
            },
        )
    };

    assert!(matches(11));
    assert!(!matches(12));
    assert!(!matches(10));
}
