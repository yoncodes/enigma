use super::*;

#[test]
fn hit_damage_percent_uses_one_resolved_hit_and_a_strict_boundary() {
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(66_810),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(66_810),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let condition = ParsedCondition {
        opcode: 41,
        type_name: "DamagePercentMoreThan".into(),
        kind: ParsedConditionKind::HitDamagePercentMoreThan {
            threshold_permille: 300,
        },
        raw_args: vec!["300".into()],
    };
    let matches = |damage, targets: &[i64]| {
        condition_matches(
            &condition,
            -1,
            targets,
            Some(&managers),
            &pool,
            TargetContext {
                hit_target_uid: -1,
                hit_resolved_damage: damage,
                ..Default::default()
            },
        )
    };

    assert!(matches(28_186, &[-1]));
    assert!(!matches(20_043, &[-1]));
    assert!(!matches(28_186, &[10]));
}

#[test]
fn player_level_reads_the_battle_player_owner() {
    let matches = |level, compare, threshold| {
        let mut managers = BattleManagers::default();
        managers.player = crate::engine::manager::player::BattlePlayerManager::new(level);
        condition_matches(
            &ParsedCondition {
                opcode: 793002,
                type_name: "Level".into(),
                kind: ParsedConditionKind::PlayerLevel { compare, threshold },
                raw_args: Vec::new(),
            },
            1,
            &[1],
            Some(&managers),
            &TargetPool::default(),
            TargetContext::default(),
        )
    };

    assert!(matches(54, ConditionCompare::GreaterThan, 40));
    assert!(matches(54, ConditionCompare::LessThan, 55));
    assert!(matches(55, ConditionCompare::GreaterThan, 40));
    assert!(!matches(55, ConditionCompare::LessThan, 55));
    assert!(matches(56, ConditionCompare::GreaterThan, 40));
    assert!(!matches(56, ConditionCompare::LessThan, 55));
}

#[test]
fn blood_pool_max_uses_runtime_state() {
    init_config();
    let condition = ParsedCondition {
        opcode: 740203,
        type_name: String::new(),
        kind: ParsedConditionKind::BloodPoolMax { min: 50, max: 999 },
        raw_args: vec!["50".into(), "999".into()],
    };

    assert!(conditions_match(
        std::slice::from_ref(&condition),
        10,
        &[10],
        None,
        &TargetPool::default(),
        TargetContext {
            blood_pool_max: 84,
            ..Default::default()
        },
    ));
    assert!(!conditions_match(
        &[condition],
        10,
        &[10],
        None,
        &TargetPool::default(),
        TargetContext::default(),
    ));
}

#[test]
fn broken_condition_reads_committed_toughness_state() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                toughness_value: Some(1),
                toughness_point: Some(1),
                is_broken: Some(false),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let condition = exact_condition(783101, "IsBroken", &[]);
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

    assert!(!matches(&managers));
    assert!(managers.toughness.reduce(10, 1, true).is_some());
    assert!(matches(&managers));
}

#[test]
fn blood_pool_value_selects_the_configured_shared_gauge() {
    init_config();
    let condition = |config_effect| ParsedCondition {
        opcode: 726304,
        type_name: "BloodPoolValue".into(),
        kind: ParsedConditionKind::BloodPoolValue {
            min: 60_000,
            max: 1_000_000,
            config_effect,
        },
        raw_args: vec!["60000".into(), "1000000".into(), config_effect.to_string()],
    };
    let context = TargetContext {
        blood_pool_value: 70_000,
        heat_scale_raw_value: 60_000,
        ..Default::default()
    };

    assert!(conditions_match(
        &[condition(0)],
        10,
        &[10],
        None,
        &TargetPool::default(),
        context,
    ));
    assert!(conditions_match(
        &[condition(1)],
        10,
        &[10],
        None,
        &TargetPool::default(),
        context,
    ));
    assert!(!conditions_match(
        &[condition(2)],
        10,
        &[10],
        None,
        &TargetPool::default(),
        context,
    ));
}

#[test]
fn ally_attacked_matches_owner_and_teammate_but_not_opponent() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(1),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let condition = ParsedCondition {
        opcode: 22213,
        type_name: String::new(),
        kind: ParsedConditionKind::AllyAttacked,
        raw_args: Vec::new(),
    };
    let matches = |hit_target_uid| {
        conditions_match(
            std::slice::from_ref(&condition),
            10,
            &[10],
            None,
            &pool,
            TargetContext {
                hit_target_uid,
                ..Default::default()
            },
        )
    };

    assert!(matches(11));
    assert!(matches(10));
    assert!(!matches(-1));
}

#[test]
fn afflatus_hit_conditions_match_only_the_attacked_target() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                career: Some(1),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    career: Some(1),
                    current_hp: Some(1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
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
    let condition = ParsedCondition {
        opcode: 47209,
        type_name: "HurtNotRestraint".into(),
        kind: ParsedConditionKind::HurtNotRestrained,
        raw_args: Vec::new(),
    };
    let matches = |hit_target_uid| {
        conditions_match(
            std::slice::from_ref(&condition),
            -1,
            &[-1],
            None,
            &pool,
            TargetContext {
                hit_source_uid: 10,
                hit_target_uid,
                ..Default::default()
            },
        )
    };

    assert!(matches(-1));
    assert!(!matches(-2));
}

#[test]
fn full_moxie_uses_each_targets_configured_maximum() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    ex_point: Some(2),
                    ex_point_max: Some(2),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    ex_point: Some(1),
                    ex_point_max: Some(2),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let condition = exact_condition(745304, "ExPointMax", &[]);

    assert!(conditions_match(
        std::slice::from_ref(&condition),
        -1,
        &[-1],
        Some(&managers),
        &pool,
        TargetContext::default(),
    ));
    assert!(!conditions_match(
        &[condition],
        -2,
        &[-2],
        Some(&managers),
        &pool,
        TargetContext::default(),
    ));
}

#[test]
fn random_condition_requires_and_compares_the_runtime_roll() {
    init_config();
    let condition = ParsedCondition {
        opcode: 552210,
        type_name: String::new(),
        kind: ParsedConditionKind::Random { threshold: 500 },
        raw_args: vec!["500".to_owned()],
    };
    let matches = |condition_random_roll| {
        conditions_match(
            std::slice::from_ref(&condition),
            10,
            &[],
            None,
            &TargetPool::default(),
            TargetContext {
                condition_random_roll,
                ..Default::default()
            },
        )
    };

    assert!(!matches(None));
    assert!(matches(Some(499)));
    assert!(!matches(Some(500)));
}

#[test]
fn follow_up_and_riposte_are_also_extra_actions() {
    init_config();
    assert!(extra_action_kind_matches(1, &[1]));
    assert!(extra_action_kind_matches(2, &[1]));
    assert!(extra_action_kind_matches(3, &[1]));
    assert!(extra_action_kind_matches(2, &[2]));
    assert!(!extra_action_kind_matches(1, &[2]));
}

#[test]
fn completed_extra_action_uses_its_configured_actor_group() {
    use crate::engine::skill::target::{TargetRequest, TargetResolver};

    init_config();
    let team = |uids: &[i64]| FightTeam {
        entitys: uids
            .iter()
            .map(|uid| FightEntityInfo {
                uid: Some(*uid),
                current_hp: Some(100),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(team(&[10, 11])),
        defender: Some(team(&[20])),
        ..Default::default()
    });
    for (opcode, name) in [(626212, "ActionSkillExtraType"), (403212, "SkillExtraType")] {
        let condition = exact_condition(opcode, name, &["3"]);
        let matches = |code, active_skill_source_uid, extra_skill_kind| {
            let context = TargetContext {
                active_skill_source_uid,
                extra_skill_kind,
                ..Default::default()
            };
            let targets = TargetResolver::resolve_with_context(
                &TargetRequest { code, raw: vec![] },
                1,
                10,
                &pool,
                &mut Default::default(),
                context,
            );
            condition_matches(&condition, 10, &targets, None, &pool, context)
        };

        assert!(matches(101, 10, 3), "{name}: owner on team");
        assert!(matches(101, 11, 3), "{name}: ally on team");
        assert!(!matches(101, 20, 3), "{name}: enemy");
        assert!(!matches(101, 10, 2), "{name}: wrong kind");
        assert!(!matches(101, 10, 0), "{name}: ordinary action");
        assert!(!matches(102, 10, 3), "{name}: owner outside group");
        assert!(matches(102, 11, 3), "{name}: other ally");
    }

    let legacy = exact_condition(403212, "SkillExtraType", &["1"]);
    let matches = |active_skill_source_uid| {
        conditions_match(
            std::slice::from_ref(&legacy),
            10,
            &[10, 11],
            None,
            &pool,
            TargetContext {
                active_skill_source_uid,
                extra_skill_kind: 1,
                ..Default::default()
            },
        )
    };
    assert!(!matches(10));
    assert!(matches(11));
}

#[test]
fn power_compare_codes_form_inclusive_config_ranges() {
    init_config();
    assert!(compare_resource(30, 1, 30));
    assert!(compare_resource(50, 1, 30));
    assert!(!compare_resource(29, 1, 30));

    assert!(compare_resource(50, 2, 69));
    assert!(compare_resource(69, 2, 69));
    assert!(!compare_resource(70, 2, 69));
}

#[test]
fn fight_progress_value_reads_committed_progress_with_config_comparisons() {
    init_config();
    let fight = Fight {
        progress_list: vec![sonettobuf::FightProgressInfo {
            id: Some(8),
            max: Some(100),
            value: Some(59),
            show_id: Some(10),
            skill_id: Some(119381117),
        }],
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let matches = |compare_code: i32, threshold: i32| {
        let compare_code = compare_code.to_string();
        let threshold = threshold.to_string();
        conditions_match(
            &[exact_condition(
                1010301,
                "ProgressValue",
                &["8", &compare_code, &threshold],
            )],
            10,
            &[10],
            Some(&managers),
            &TargetPool::default(),
            TargetContext::default(),
        )
    };

    assert!(matches(1, 30));
    assert!(matches(2, 59));
    assert!(matches(3, 59));
    assert!(!matches(3, 100));
}

#[test]
fn hurt_kind_reads_the_attacker_damage_type() {
    init_config();
    let reality = ParsedCondition {
        opcode: 20209,
        type_name: "HurtReal".into(),
        kind: ParsedConditionKind::AttackerDamageType(
            crate::engine::skill::target::EntityDamageType::Reality,
        ),
        raw_args: Vec::new(),
    };
    let mental = ParsedCondition {
        opcode: 21209,
        type_name: "HurtMagic".into(),
        kind: ParsedConditionKind::AttackerDamageType(
            crate::engine::skill::target::EntityDamageType::Mental,
        ),
        raw_args: Vec::new(),
    };
    let mut attacker = TargetEntity::default();
    attacker.uid = 10;
    attacker.damage_type = crate::engine::skill::target::EntityDamageType::Reality;
    let mut pool = TargetPool::default();
    pool.attacker_main.push(attacker.clone());
    pool.attacker_all.push(attacker);
    let context = TargetContext {
        hit_source_uid: 10,
        ..Default::default()
    };

    assert!(conditions_match(
        &[reality],
        -1,
        &[-1],
        None,
        &pool,
        context
    ));
    assert!(!conditions_match(
        &[mental],
        -1,
        &[-1],
        None,
        &pool,
        context
    ));
}

#[test]
fn hero_damage_type_reads_the_setup_skill_source() {
    init_config();
    let reality = exact_condition(36021, "HeroReal", &[]);
    let mental = exact_condition(37021, "HeroMagic", &[]);
    for (damage_type, is_reality) in [
        (
            crate::engine::skill::target::EntityDamageType::Reality,
            true,
        ),
        (
            crate::engine::skill::target::EntityDamageType::Mental,
            false,
        ),
    ] {
        let mut source = TargetEntity::default();
        source.uid = 10;
        source.damage_type = damage_type;
        let mut pool = TargetPool::default();
        pool.attacker_main.push(source.clone());
        pool.attacker_all.push(source);
        let matches = |condition| {
            conditions_match(
                std::slice::from_ref(condition),
                10,
                &[10],
                None,
                &pool,
                TargetContext::default(),
            )
        };
        assert_eq!(matches(&reality), is_reality);
        assert_eq!(matches(&mental), !is_reality);
    }
}

#[test]
fn alive_team_count_reads_manager_hp_not_the_fight_snapshot() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![10, 11]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    current_hp: Some(100),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.hp.lose(11, 100, -1);
    let condition = ParsedCondition {
        opcode: 616012,
        type_name: "TeammateAliveNumNoSp".into(),
        kind: ParsedConditionKind::EntityCount {
            scope: EntityCountScope::AliveTeammatesNoSp,
            compare: ConditionCompare::Equal,
            count: 1,
        },
        raw_args: vec!["1".into()],
    };

    assert!(conditions_match(
        &[condition],
        10,
        &[10],
        Some(&managers),
        &pool,
        TargetContext::default(),
    ));
}

#[test]
fn other_teammate_count_excludes_the_alive_source() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![10, 11]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    current_hp: Some(100),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.hp.lose(11, 100, -1);

    assert!(conditions_match(
        &[exact_condition(73301, "TeammateAliveNum", &["0"])],
        10,
        &[10],
        Some(&managers),
        &pool,
        TargetContext::default(),
    ));
}

#[test]
fn battle_tag_count_uses_alive_members_of_the_casters_team() {
    init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        entity_type: Some(1),
        current_hp: Some(100),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 3127), entity(11, 3134)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 3139)],
            ..Default::default()
        }),
        ..Default::default()
    });
    let condition = ParsedCondition {
        opcode: 762021,
        type_name: "BattleTagNum".into(),
        kind: ParsedConditionKind::BattleTagCount {
            tag_id: 114,
            compare: ConditionCompare::Equal,
            threshold: 2,
        },
        raw_args: vec!["114".into(), "2".into(), "3".into()],
    };

    assert!(conditions_match(
        &[condition],
        10,
        &[10],
        None,
        &pool,
        TargetContext::default(),
    ));
}

#[test]
fn battle_tag_check_uses_the_condition_target() {
    init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        entity_type: Some(1),
        current_hp: Some(100),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 3146), entity(11, 3149)],
            ..Default::default()
        }),
        ..Default::default()
    });
    let condition = exact_condition(1021002, "BattleTagCheck", &["102"]);

    assert!(conditions_match(
        std::slice::from_ref(&condition),
        10,
        &[10],
        None,
        &pool,
        TargetContext::default(),
    ));
    assert!(!conditions_match(
        &[condition],
        10,
        &[11],
        None,
        &pool,
        TargetContext::default(),
    ));
}

#[test]
fn all_rounder_counts_for_the_configured_tag_conditions() {
    init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(100),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 3141), entity(11, 3146)],
            ..Default::default()
        }),
        ..Default::default()
    });
    assert!(condition_matches(
        &exact_condition(762005, "BattleTagNum", &["113", "2", "3"]),
        10,
        &[10],
        None,
        &pool,
        TargetContext::default(),
    ));
    let conduit = exact_condition(1021101, "BattleTagCheck", &["131"]);
    assert!(condition_matches(
        &conduit,
        10,
        &[11],
        None,
        &pool,
        TargetContext::default()
    ));
    assert!(!condition_matches(
        &conduit,
        10,
        &[10],
        None,
        &pool,
        TargetContext::default()
    ));
}
