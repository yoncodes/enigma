use super::*;

#[test]
fn source_life_less_uses_a_strict_hp_percentage_threshold() {
    init_config();
    let condition = exact_condition(1201, "LifeLess", &["500"]);
    let matches = |current_hp| {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(current_hp),
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
        let managers = BattleManagers::seeded(&fight);
        condition_matches(
            &condition,
            10,
            &[10],
            Some(&managers),
            &TargetPool::from_fight(&fight),
            TargetContext::default(),
        )
    };

    assert!(matches(499));
    assert!(!matches(500));
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
fn from_and_to_buff_needs_the_attacker_from_buff_and_the_owner_to_buff() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(2292031),
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
                    buff_id: Some(229103),
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
