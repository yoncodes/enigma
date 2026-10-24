use super::*;

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
fn alive_allied_model_count_uses_the_casters_live_team() {
    init_config();
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3123),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    model_id: Some(103370111),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    model_id: Some(103370112),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-3),
                    model_id: Some(103370112),
                    current_hp: Some(0),
                    ..Default::default()
                },
            ],
            sub_entitys: vec![FightEntityInfo {
                uid: Some(-4),
                model_id: Some(103370112),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let condition = exact_condition(74210, "MonsterAliveNumMoreThan", &["1", "103370112", "1"]);

    assert!(condition_matches(
        &condition,
        -1,
        &[-1],
        None,
        &pool,
        TargetContext::default(),
    ));
    let two_required = exact_condition(74210, "MonsterAliveNumMoreThan", &["1", "103370112", "2"]);
    assert!(!condition_matches(
        &two_required,
        -1,
        &[-1],
        None,
        &pool,
        TargetContext::default(),
    ));
    assert!(!condition_matches(
        &condition,
        10,
        &[10],
        None,
        &pool,
        TargetContext::default(),
    ));
}
