use super::*;

#[test]
fn exact_target_include_hero_matches_only_the_configured_model() {
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3092),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    model_id: Some(3102),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(12),
                    model_id: Some(3121),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    });
    let condition = exact_condition(595101, "TargetIncludeHero", &["3102"]);

    assert!(condition_matches(
        &condition,
        10,
        &[11],
        None,
        &pool,
        TargetContext::default(),
    ));
    assert!(!condition_matches(
        &condition,
        10,
        &[12],
        None,
        &pool,
        TargetContext::default(),
    ));
}

#[test]
fn exact_after_hit_target_include_hero_matches_the_active_source_not_the_target() {
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3092),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    model_id: Some(3102),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(12),
                    model_id: Some(3121),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(1083030051),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let condition = exact_condition(595210, "TargetIncludeHero", &["3102"]);
    let matches = |active_skill_source_uid| {
        condition_matches(
            &condition,
            10,
            &[active_skill_source_uid],
            None,
            &pool,
            TargetContext {
                active_skill_source_uid,
                runtime_target_uid: -1,
                ..Default::default()
            },
        )
    };

    assert!(matches(11));
    assert!(!matches(12));
    assert!(!matches(0));
}

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
fn after_hit_ultimate_level_751210_matches_the_captured_rank_set() {
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    ex_skill_level: Some(0),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    ex_skill_level: Some(5),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    });
    let lower_ranks = exact_condition(751210, "ExSkillLevel", &["0,1,2,3,4"]);
    let max_rank = exact_condition(751210, "ExSkillLevel", &["5"]);

    assert!(condition_matches(
        &lower_ranks,
        10,
        &[10],
        None,
        &pool,
        TargetContext::default(),
    ));
    assert!(!condition_matches(
        &max_rank,
        10,
        &[10],
        None,
        &pool,
        TargetContext::default(),
    ));
    assert!(condition_matches(
        &max_rank,
        10,
        &[11],
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
fn received_hit_afflatus_conditions_use_the_resolved_result() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                career: Some(3),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                career: Some(8),
                weak_careers: vec![1],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let matches = |condition: ParsedCondition, hit_career_restraint| {
        conditions_match(
            &[condition],
            -1,
            &[-1],
            None,
            &pool,
            TargetContext {
                hit_source_uid: 10,
                hit_target_uid: -1,
                hit_career_restraint: Some(hit_career_restraint),
                ..Default::default()
            },
        )
    };

    assert!(matches(exact_condition(33209, "HurtRestraint", &[]), true));
    assert!(!matches(
        exact_condition(47209, "HurtNotRestraint", &[]),
        true
    ));
    assert!(!matches(
        exact_condition(33209, "HurtRestraint", &[]),
        false
    ));
    assert!(matches(
        exact_condition(47209, "HurtNotRestraint", &[]),
        false
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
fn per_team_career_condition_counts_the_carrier() {
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    career: Some(2),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    career: Some(2),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(12),
                    career: Some(3),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    });
    let condition = exact_condition(517010, "PerHasTargetCareer", &["2", "1", "1"]);

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
fn exact_target_team_career_count_selects_the_captured_three_member_branch() {
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    career: Some(4),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    career: Some(4),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(12),
                    career: Some(4),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(13),
                    career: Some(1),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    });
    let equal_two = exact_condition(516101, "HasTargetCareerNum", &["4", "3", "2", "1"]);
    let at_least_three = exact_condition(516101, "HasTargetCareerNum", &["4", "1", "3", "1"]);

    assert!(!condition_matches(
        &equal_two,
        10,
        &[10],
        None,
        &pool,
        TargetContext::default(),
    ));
    assert!(condition_matches(
        &at_least_three,
        10,
        &[10],
        None,
        &pool,
        TargetContext::default(),
    ));
}
