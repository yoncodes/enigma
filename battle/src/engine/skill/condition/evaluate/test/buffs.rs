use super::*;

#[test]
fn exact_buff_id_condition_does_not_match_a_buff_type() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                buffs: vec![BuffInfo {
                    buff_id: Some(26030),
                    duration: Some(1),
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
        opcode: 19205,
        type_name: "HasBuffId".into(),
        kind: ParsedConditionKind::BuffId {
            mode: BuffConditionMode::ExactPresent,
            buff_ids: vec![5022],
        },
        raw_args: vec!["5022".into()],
    };

    assert!(managers.buff.has_active_buff_id_or_type(10, 5022));
    assert!(!conditions_match(
        &[condition],
        10,
        &[10],
        Some(&managers),
        &TargetPool::from_fight(&fight),
        TargetContext::default(),
    ));
}

#[test]
fn repeated_absence_conditions_require_every_buff_to_be_absent() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1),
                buffs: vec![BuffInfo {
                    buff_id: Some(530000111),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let conditions = [530000111, 530000112].map(|buff_id| ParsedCondition {
        opcode: 57104,
        type_name: String::new(),
        kind: ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Absent,
            buff_ids: vec![buff_id],
        },

        raw_args: vec![buff_id.to_string()],
    });

    assert!(!conditions_match(
        &conditions,
        -1,
        &[-1],
        Some(&managers),
        &TargetPool::from_fight(&fight),
        TargetContext::default(),
    ));

    let managers = BattleManagers::seeded(&Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    assert!(conditions_match(
        &conditions,
        -1,
        &[-1],
        Some(&managers),
        &TargetPool::from_fight(&fight),
        TargetContext::default(),
    ));
}

#[test]
fn master_halo_requires_active_state_not_an_owned_passive_definition() {
    init_config();
    let fight_with = |buffs| Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                passive_skill: vec![30860161],
                buffs,
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let condition = ParsedCondition {
        opcode: 701201,
        type_name: "HasMasterHalo".into(),
        kind: ParsedConditionKind::MasterHalo,
        raw_args: Vec::new(),
    };
    let matches = |fight: &Fight| {
        let managers = BattleManagers::seeded(fight);
        conditions_match(
            std::slice::from_ref(&condition),
            10,
            &[10],
            Some(&managers),
            &TargetPool::from_fight(fight),
            TargetContext::default(),
        )
    };

    assert!(!matches(&fight_with(Vec::new())));
    assert!(matches(&fight_with(vec![BuffInfo {
        buff_id: Some(30860161),
        ..Default::default()
    }])));
}

#[test]
fn final_settlement_buff_threshold_reads_accumulated_layers() {
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
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let condition = exact_condition(581307, "AccAddBuffCountByBuffId", &["118353082", "5"]);
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

    for _ in 0..4 {
        managers.buff.add(&managers.hp, 10, 10, 118353082, 1);
    }
    assert!(!matches(&managers));
    managers.buff.add(&managers.hp, 10, 10, 118353082, 1);
    assert!(matches(&managers));
}

#[test]
fn player_buff_condition_reads_the_configured_team() {
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
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let condition = exact_condition(750101, "PlayerHasBuff", &["2", "0", "109320002"]);
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
    managers.buff.add(&managers.hp, 10, -1, 109320002, 1);
    assert!(!matches(&managers));
}

#[test]
fn career_check_parameter_selects_share_or_not_share() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    career: Some(1),
                    current_hp: Some(1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    career: Some(1),
                    current_hp: Some(1),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let condition = |param| ParsedCondition {
        opcode: 508104,
        type_name: String::new(),
        kind: ParsedConditionKind::TargetSharesCasterCareer { param },
        raw_args: vec![param.to_string()],
    };

    assert!(conditions_match(
        &[condition(0)],
        10,
        &[11],
        None,
        &pool,
        TargetContext::default(),
    ));
    assert!(!conditions_match(
        &[condition(1)],
        10,
        &[11],
        None,
        &pool,
        TargetContext::default(),
    ));
}
