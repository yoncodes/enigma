use super::*;

#[test]
fn current_card_enchant_preserves_exact_ids_and_supports_any_rewrite() {
    init_config();
    let condition = |enchant_id| ParsedCondition {
        opcode: 760212,
        type_name: "CurUseCardEnchant".into(),
        kind: ParsedConditionKind::CurrentCardEnchant { enchant_id },
        raw_args: vec![enchant_id.to_string()],
    };

    assert_eq!(
        satisfied_card_enchants(&[condition(0)], &[10_010]),
        vec![ParsedCondition::always()]
    );
    assert_eq!(
        satisfied_card_enchants(&[condition(10_010)], &[10_010]),
        vec![ParsedCondition::always()]
    );
    assert_eq!(
        satisfied_card_enchants(&[condition(10_011)], &[10_010]),
        vec![condition(10_011)]
    );
    assert_eq!(
        satisfied_card_enchants(&[condition(0)], &[]),
        vec![condition(0)]
    );
}

#[test]
fn single_kill_count_reads_the_current_action_state() {
    init_config();
    let condition = ParsedCondition {
        opcode: 11210,
        type_name: "SingleKillNum".into(),
        kind: ParsedConditionKind::SingleKillCount { threshold: 2 },
        raw_args: vec!["2".into()],
    };
    let matches = |action_kill_count| {
        conditions_match(
            std::slice::from_ref(&condition),
            10,
            &[10],
            None,
            &TargetPool::default(),
            TargetContext {
                action_kill_count,
                ..Default::default()
            },
        )
    };

    assert!(!matches(1));
    assert!(matches(2));
}

#[test]
fn target_guard_broken_reads_the_current_action_state() {
    init_config();
    let condition = ParsedCondition {
        opcode: 791210,
        type_name: "ToBrokenEnemy".into(),
        kind: ParsedConditionKind::TargetGuardBroken,
        raw_args: Vec::new(),
    };
    let matches = |action_guard_break_count| {
        conditions_match(
            std::slice::from_ref(&condition),
            10,
            &[10],
            None,
            &TargetPool::default(),
            TargetContext {
                action_guard_break_count,
                ..Default::default()
            },
        )
    };

    assert!(!matches(0));
    assert!(matches(1));
}

#[test]
fn guard_broken_only_matches_the_entity_from_the_break_event() {
    init_config();
    let condition = ParsedCondition {
        opcode: 2092,
        type_name: "None".into(),
        kind: ParsedConditionKind::GuardBroken,
        raw_args: Vec::new(),
    };
    let matches = |condition_target, toughness_broken_uid| {
        conditions_match(
            std::slice::from_ref(&condition),
            condition_target,
            &[condition_target],
            None,
            &TargetPool::default(),
            TargetContext {
                toughness_broken_uid,
                ..Default::default()
            },
        )
    };

    assert!(matches(-1, -1));
    assert!(!matches(-1, -2));
}

#[test]
fn per_kill_count_repeats_once_for_each_kill() {
    init_config();
    let condition = ParsedCondition {
        opcode: 99210,
        type_name: "PerKillNum".into(),
        kind: ParsedConditionKind::PerKillCount { divisor: 1 },
        raw_args: vec!["1".into()],
    };
    let fires = |action_kill_count| {
        conditions_fire_count(
            std::slice::from_ref(&condition),
            10,
            &[10],
            None,
            &TargetPool::default(),
            TargetContext {
                action_kill_count,
                ..Default::default()
            },
        )
    };

    assert_eq!(fires(0), 0);
    assert_eq!(fires(2), 2);
}

#[test]
fn per_hp_repeats_for_each_complete_target_hp_interval() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1_000),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let condition = ParsedCondition {
        opcode: 744203,
        type_name: "PerHp".into(),
        kind: ParsedConditionKind::PerHp {
            interval_permille: 200,
        },
        raw_args: vec!["200".into()],
    };
    let fires = |managers: &BattleManagers| {
        conditions_fire_count(
            std::slice::from_ref(&condition),
            10,
            &[-1],
            Some(managers),
            &pool,
            TargetContext::default(),
        )
    };

    assert_eq!(fires(&managers), 5);
    managers.hp.lose(-1, 1, -1);
    assert_eq!(fires(&managers), 4);
    managers.hp.lose(-1, 799, -1);
    assert_eq!(fires(&managers), 1);
}

#[test]
fn empty_condition_list_fires_once() {
    init_config();
    assert_eq!(
        conditions_fire_count(
            &[],
            10,
            &[10],
            None,
            &TargetPool::default(),
            TargetContext::default(),
        ),
        1
    );
}

#[test]
fn boolean_guards_do_not_collapse_a_repeat_count() {
    init_config();
    let active_skill = ParsedCondition {
        opcode: 507201,
        type_name: "UseSkillId".into(),
        kind: ParsedConditionKind::ActiveSkillId(vec![308801711]),
        raw_args: vec!["308801711".into()],
    };
    let injury_count = ParsedCondition {
        opcode: 578,
        type_name: "TeamInjuryCountRound".into(),
        kind: ParsedConditionKind::TeamInjuryCountRound { max_count: 20 },
        raw_args: vec!["20".into()],
    };

    assert_eq!(
        conditions_fire_count(
            &[active_skill, injury_count],
            10,
            &[10],
            None,
            &TargetPool::default(),
            TargetContext {
                active_skill_id: 308801711,
                team_injury_count_round: 7,
                ..Default::default()
            },
        ),
        7
    );
}

#[test]
fn per_buff_type_layer_preserves_its_stack_count() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    buff_id: Some(31340002),
                    layer: Some(7),
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
        opcode: 518203,
        type_name: "PerHasBuffTypeLayer".into(),
        kind: ParsedConditionKind::PerBuffTypeLayer {
            type_ids: vec![31340002],
            min: 1,
            max: 20,
        },
        raw_args: vec!["1".into(), "20".into(), "31340002".into()],
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
        7
    );
}

#[test]
fn team_buff_type_gate_requires_zero_layers_across_all_targets() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-3),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let condition = ParsedCondition {
        opcode: 514100,
        type_name: "SelfTeamHasBuffTypeLayerLessThan".into(),
        kind: ParsedConditionKind::BuffTypeCount {
            type_ids: vec![30650104],
            compare: ConditionCompare::LessThanOrEqual,
            threshold: 0,
        },
        raw_args: vec!["0".into(), "30650104".into()],
    };
    let matches = |managers: &BattleManagers| {
        conditions_match(
            std::slice::from_ref(&condition),
            10,
            &[-2, -3],
            Some(managers),
            &TargetPool::from_fight(&fight),
            TargetContext::default(),
        )
    };

    assert!(matches(&managers));
    managers.buff.add(&managers.hp, 10, -3, 30650204, 1);
    assert!(!matches(&managers));
}

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
        opcode: 61203,
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
