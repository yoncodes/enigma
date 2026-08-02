use super::*;

#[test]
fn owner_incantation_rank_rejects_an_ally_action() {
    init_config();
    let condition = ParsedCondition {
        opcode: 659212,
        type_name: "UseSkill".into(),
        kind: ParsedConditionKind::UseSkillRank(vec![1, 2, 3]),
        raw_args: vec!["1,2,3".into()],
    };
    let matches = |active_skill_source_uid, active_skill_rank| {
        conditions_match(
            std::slice::from_ref(&condition),
            10,
            &[10],
            None,
            &TargetPool::default(),
            TargetContext {
                active_skill_source_uid,
                active_skill_rank,
                ..Default::default()
            },
        )
    };

    assert!(!matches(11, 1));
    assert!(!matches(10, 0));
    assert!(matches(10, 1));
    assert!(matches(10, 2));
    assert!(matches(10, 3));
}

#[test]
fn synthetic_emitter_is_not_an_active_incantation_user() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let matches = |kind, active_skill_source_uid, extra_skill_kind| {
        condition_matches(
            &ParsedCondition {
                opcode: 502212,
                type_name: String::new(),
                kind,
                raw_args: Vec::new(),
            },
            10,
            &[10],
            None,
            &pool,
            TargetContext {
                active_skill_source_uid,
                active_skill_is_attack: true,
                extra_skill_kind,
                direct_skill_body: true,
                ..Default::default()
            },
        )
    };

    assert!(matches(
        ParsedConditionKind::ActiveUseSkill { slot: 0 },
        10,
        0
    ));
    assert!(!matches(
        ParsedConditionKind::ActiveUseSkill { slot: 0 },
        10,
        crate::engine::skill::condition::extra::ExtraSkillKind::FollowUp.id()
    ));
    assert!(matches(ParsedConditionKind::UseHurtSkill, 10, 0));
    assert!(!matches(
        ParsedConditionKind::ActiveUseSkill { slot: 0 },
        crate::engine::manager::emitter::UID,
        0
    ));
    assert!(!matches(
        ParsedConditionKind::UseHurtSkill,
        crate::engine::manager::emitter::UID,
        0
    ));
}

#[test]
fn hero_round_interval_alternates_from_its_configured_start_round() {
    init_config();
    let matches = |start_round, current_round| {
        let condition = ParsedCondition {
            opcode: 45104,
            type_name: "HeroRoundInterval".into(),
            kind: ParsedConditionKind::RoundInterval {
                start_round,
                period: 2,
            },
            raw_args: vec!["2".into(), start_round.to_string()],
        };
        condition_matches(
            &condition,
            -1,
            &[-1],
            None,
            &TargetPool::default(),
            TargetContext {
                current_round,
                ..Default::default()
            },
        )
    };

    assert!(matches(1, 1));
    assert!(!matches(1, 2));
    assert!(matches(1, 3));
    assert!(!matches(2, 1));
    assert!(matches(2, 2));
    assert!(!matches(2, 3));
}

#[test]
fn use_ex_skill_checks_the_active_skill_payload() {
    init_config();
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3134),
                ex_skill: Some(31345131),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let condition = ParsedCondition {
        opcode: 25210,
        type_name: String::new(),
        kind: ParsedConditionKind::UseExSkill,
        raw_args: Vec::new(),
    };
    let matches = |active_skill_id| {
        condition_matches(
            &condition,
            10,
            &[10],
            None,
            &pool,
            TargetContext {
                active_skill_id,
                ..Default::default()
            },
        )
    };

    assert!(matches(31345131));
    assert!(matches(31340131));
    assert!(!matches(31345111));
}

#[test]
fn negated_use_ex_skill_matches_basic_incantations_only() {
    init_config();
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3134),
                ex_skill: Some(31345131),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let condition = ParsedCondition {
        opcode: 25208,
        type_name: "UseExSkill".into(),
        kind: ParsedConditionKind::Not(Box::new(ParsedConditionKind::UseExSkill)),
        raw_args: Vec::new(),
    };
    let matches = |active_skill_id| {
        condition_matches(
            &condition,
            10,
            &[10],
            None,
            &pool,
            TargetContext {
                active_skill_id,
                ..Default::default()
            },
        )
    };

    assert!(matches(31345111));
    assert!(!matches(31345131));
}

#[test]
fn use_ex_skill_does_not_treat_an_enhanced_basic_skill_as_an_ultimate() {
    init_config();
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3086),
                ex_skill: Some(30865234),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let condition = ParsedCondition {
        opcode: 25210,
        type_name: "UseExSkill".into(),
        kind: ParsedConditionKind::UseExSkill,
        raw_args: Vec::new(),
    };
    let matches = |active_skill_id| {
        condition_matches(
            &condition,
            10,
            &[10],
            None,
            &pool,
            TargetContext {
                active_skill_id,
                ..Default::default()
            },
        )
    };

    assert!(matches(30865234));
    assert!(!matches(30865117));
}

#[test]
fn teammate_ex_skill_requires_the_other_ally_as_runtime_source() {
    init_config();
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    ex_skill: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    ex_skill: Some(200),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    });
    let condition = ParsedCondition {
        opcode: 720212,
        type_name: "TeammateUseExSkill".into(),
        kind: ParsedConditionKind::TeammateUseExSkill,
        raw_args: Vec::new(),
    };
    let matches = |active_skill_source_uid, active_skill_id| {
        condition_matches(
            &condition,
            10,
            &[10],
            None,
            &pool,
            TargetContext {
                active_skill_source_uid,
                active_skill_id,
                ..Default::default()
            },
        )
    };

    assert!(matches(11, 200));
    assert!(!matches(10, 100));
    assert!(!matches(11, 100));
}

#[test]
fn target_use_ex_skill_requires_the_selected_runtime_actor() {
    init_config();
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    ex_skill: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    ex_skill: Some(200),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    });
    let condition = ParsedCondition {
        opcode: 25212,
        type_name: "UseExSkill".into(),
        kind: ParsedConditionKind::TargetUseExSkill,
        raw_args: Vec::new(),
    };
    let matches = |condition_targets: &[i64], active_skill_source_uid, active_skill_id| {
        condition_matches(
            &condition,
            10,
            condition_targets,
            None,
            &pool,
            TargetContext {
                active_skill_source_uid,
                active_skill_id,
                ..Default::default()
            },
        )
    };

    assert!(matches(&[11], 11, 200));
    assert!(!matches(&[11], 10, 100));
    assert!(!matches(&[11], 11, 100));
    assert!(matches(&[10], 10, 100));
}

#[test]
fn ally_action_targets_excluding_the_observer_scope_the_action_actor() {
    init_config();
    let condition = ParsedCondition {
        opcode: 212,
        type_name: "None".into(),
        kind: ParsedConditionKind::None(
            crate::engine::skill::condition::none::NoneMode::AllyAction,
        ),
        raw_args: Vec::new(),
    };
    let condition = satisfied_condition(&condition, DefinitionKey::new(212, "None"));
    let matches = |condition_targets: &[i64], active_skill_source_uid| {
        condition_matches(
            &condition,
            10,
            condition_targets,
            None,
            &TargetPool::default(),
            TargetContext {
                active_skill_source_uid,
                ..Default::default()
            },
        )
    };

    assert!(!matches(&[11], 10));
    assert!(matches(&[11], 11));
    assert!(matches(&[10], 11));
}

#[test]
fn ally_ultimate_and_summon_selectors_use_runtime_owners() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3074),
                    skill_group1: vec![30740121],
                    ex_skill: Some(30740141),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    ex_skill: Some(999),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.summon.add(10, 10, 150011, 1, 1);
    managers.summon.add(10, 10, 150021, 1, 1);
    let matches = |kind, condition_targets: &[i64], active_skill_source_uid, active_skill_id| {
        condition_matches(
            &ParsedCondition {
                opcode: 25212,
                type_name: String::new(),
                kind,
                raw_args: Vec::new(),
            },
            10,
            condition_targets,
            Some(&managers),
            &pool,
            TargetContext {
                active_skill_source_uid,
                active_skill_id,
                active_skill_rank: 1,
                ..Default::default()
            },
        )
    };

    assert!(matches(
        ParsedConditionKind::TargetUseExSkill,
        &[11],
        11,
        999
    ));
    assert!(!matches(
        ParsedConditionKind::TargetUseExSkill,
        &[10],
        11,
        999
    ));
    assert!(matches(
        ParsedConditionKind::SpecificSkill { group: 1, rank: 0 },
        &[10],
        10,
        30740121
    ));
    assert!(matches(
        ParsedConditionKind::SpecificSkill { group: 4, rank: 0 },
        &[10],
        10,
        30740121
    ));
    assert!(matches(
        ParsedConditionKind::SpecificSkill { group: 3, rank: 0 },
        &[10],
        10,
        30740141
    ));
    assert!(matches(
        ParsedConditionKind::SpecificSkill { group: 5, rank: 1 },
        &[10],
        10,
        30740121
    ));
    assert!(matches(
        ParsedConditionKind::GroupSummonedCount {
            owner_model_id: 3074,
            required_level: 0,
            compare: ConditionCompare::Equal,
            count: 2,
        },
        &[10],
        10,
        0
    ));
}
