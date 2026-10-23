use super::*;

#[test]
fn burn_overflow_repeats_once_per_rejected_layer() {
    init_config();
    let condition = ParsedCondition {
        opcode: 564203,
        type_name: "BurnOverflow".into(),
        kind: ParsedConditionKind::BurnOverflow,
        raw_args: Vec::new(),
    };

    assert_eq!(
        conditions_fire_count(
            &[condition],
            10,
            &[-1],
            None,
            &TargetPool::default(),
            TargetContext {
                buff_overflow_amount: 4,
                ..Default::default()
            },
        ),
        4
    );
}

#[test]
fn conduit_counter_repeats_are_team_scoped_capped_and_reset() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3144),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(20),
                model_id: Some(3144),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60297, "AddDeviceCounter"),
    };
    for (source_uid, team, kind, delta) in [
        (10, 1, ConduitCounterKind::EnergyAccumulation, 25),
        (10, 1, ConduitCounterKind::Activation, 7),
        (20, 2, ConduitCounterKind::EnergyAccumulation, 3),
    ] {
        managers
            .conduit
            .execute(ConduitCommand::ChangeCounter(ConduitCounterChange {
                origin,
                source_uid,
                team,
                skill_id: 0,
                kind,
                delta,
            }))
            .unwrap();
    }
    let condition = |kind, divisor, max_count| {
        let kind_arg = match kind {
            ConduitCounterKind::EnergyAccumulation => 1,
            ConduitCounterKind::Activation => 2,
        };
        ParsedCondition {
            opcode: 786203,
            type_name: "PerDeviceCounter".into(),
            kind: ParsedConditionKind::PerConduitCounter {
                kind,
                divisor,
                max_count,
            },
            raw_args: vec![
                kind_arg.to_string(),
                divisor.to_string(),
                max_count.to_string(),
            ],
        }
    };
    let fire_count = |source_uid, condition: ParsedCondition, managers: &BattleManagers| {
        conditions_fire_count(
            &[condition],
            source_uid,
            &[source_uid],
            Some(managers),
            &pool,
            TargetContext::default(),
        )
    };

    assert_eq!(
        fire_count(
            10,
            condition(ConduitCounterKind::EnergyAccumulation, 2, 10),
            &managers,
        ),
        10
    );
    assert_eq!(
        fire_count(
            20,
            condition(ConduitCounterKind::EnergyAccumulation, 2, 10),
            &managers,
        ),
        1
    );
    assert_eq!(
        fire_count(
            10,
            condition(ConduitCounterKind::Activation, 1, 20),
            &managers,
        ),
        7
    );
    assert_eq!(
        fire_count(
            20,
            condition(ConduitCounterKind::Activation, 1, 20),
            &managers,
        ),
        0
    );

    managers.conduit.begin_round();
    assert_eq!(
        fire_count(
            10,
            condition(ConduitCounterKind::EnergyAccumulation, 1, 20),
            &managers,
        ),
        0
    );
}

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
        opcode: 992101,
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
fn team_entity_exit_matches_a_dead_enemy_relative_to_the_source() {
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
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(0),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let matches = |source_uid| {
        conditions_match(
            &[ParsedCondition {
                opcode: 613403,
                type_name: "PerTeamEntityExitCount".into(),
                kind: ParsedConditionKind::TeamEntityExited { max_count: 2 },
                raw_args: vec!["2".into(), "2".into()],
            }],
            source_uid,
            &[-1],
            Some(&managers),
            &pool,
            TargetContext {
                runtime_target_uid: -1,
                ..Default::default()
            },
        )
    };

    assert!(matches(10));
    assert!(!matches(-1));
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
fn lost_life_per_repeats_for_each_complete_missing_hp_interval() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(750),
                    attr: Some(sonettobuf::HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(1_000),
                    attr: Some(sonettobuf::HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let condition = ParsedCondition {
        opcode: 12203,
        type_name: "LostLifePer".into(),
        kind: ParsedConditionKind::PerLostHp {
            interval_permille: 100,
        },
        raw_args: vec!["100".into()],
    };

    assert_eq!(
        conditions_fire_count(
            std::slice::from_ref(&condition),
            10,
            &[-1, -2],
            Some(&managers),
            &pool,
            TargetContext {
                hit_target_uid: -1,
                ..Default::default()
            },
        ),
        2
    );
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
