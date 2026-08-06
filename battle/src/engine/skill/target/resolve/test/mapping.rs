use super::*;

#[test]
fn unmapped_target_codes_are_detectable_before_capture_override() {
    assert!(is_mapped_target_code(
        crate::engine::skill::target::request::SOURCE_TARGET_CODE
    ));
    assert!(is_mapped_target_code(128));
    assert!(is_mapped_target_code(208));
    assert!(!is_mapped_target_code(999));
}

#[test]
fn overlapping_enemy_codes_keep_their_exact_rules() {
    assert_eq!(target_rule(210), Some(TargetRule::LowestHpPercentageEnemy));
    assert_eq!(target_rule(221), Some(TargetRule::PriorityBossEnemy));
    assert_eq!(target_rule(231), Some(TargetRule::EnemyPosition(2)));
    assert_eq!(target_rule(233), Some(TargetRule::Runtime));
    assert_eq!(target_rule(232), Some(TargetRule::EnemyPosition(7)));
    assert_eq!(target_rule(234), Some(TargetRule::AssistBoss));
}

#[test]
fn target_203_selects_the_event_source() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity_at(-10, 1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);

    assert_eq!(
        resolve_code_with_context(
            203,
            -10,
            &pool,
            &mut RoundDeterminism::default(),
            TargetContext {
                runtime_target_uid: -10,
                event_source_uid: 10,
                ..Default::default()
            },
        ),
        vec![10]
    );
}

#[test]
fn adjacent_front_and_behind_targets_keep_their_direction() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1), entity_at(11, 2), entity_at(12, 3)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);

    assert_eq!(
        resolve_code(128, 11, &pool, &mut RoundDeterminism::default()),
        vec![10]
    );
    assert_eq!(
        resolve_code(129, 11, &pool, &mut RoundDeterminism::default()),
        vec![12]
    );
}

#[test]
fn target_131_selects_a_random_other_ally() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![entity_at(-3, 1), entity_at(-2, 2), entity_at(-1, 3)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_skill_target_choices([SkillTargetChoice {
        skill_id: 1001,
        source_uid: -3,
        target_code: 131,
        targets: vec![-1],
        additional_targets: Vec::new(),
        crit_targets: Vec::new(),
        additional_crit_targets: Vec::new(),
    }]);

    assert_eq!(resolve_code(131, -3, &pool, &mut determinism), vec![-1]);
}

#[test]
fn first_ally_slot_target_selects_position_one() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1), entity_at(11, 2), entity_at(12, 3)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);

    assert_eq!(
        resolve_code(124, 0, &pool, &mut RoundDeterminism::default()),
        vec![10]
    );
}

#[test]
fn priority_boss_target_selects_the_first_configured_boss() {
    init_config();
    let enemy = |uid, model_id, position| FightEntityInfo {
        model_id: Some(model_id),
        ..entity_at(uid, position)
    };
    let fight = Fight {
        episode_id: Some(90001601),
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![enemy(-2, 900016101, 1), enemy(-3, 900016102, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);

    assert_eq!(
        resolve_code(221, 10, &pool, &mut RoundDeterminism::default()),
        vec![-2]
    );
}

#[test]
fn lowest_hp_percentage_enemy_uses_current_hp_ratio() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                entity_stats(-2, 1, 40, 100, 0),
                entity_stats(-3, 2, 60, 200, 0),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);

    assert_eq!(
        TargetResolver::resolve(
            &TargetRequest {
                code: 210,
                raw: Vec::new(),
            },
            40,
            10,
            &pool,
            &mut RoundDeterminism::default(),
        ),
        vec![-3]
    );
}

#[test]
fn target_207_selects_the_enemy_with_the_highest_attack() {
    init_config();
    let enemy = |uid, position, attack| FightEntityInfo {
        attr: Some(HeroAttribute {
            hp: Some(100),
            attack: Some(attack),
            ..Default::default()
        }),
        ..entity_at(uid, position)
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![enemy(10, 1, 300), enemy(11, 2, 700), enemy(12, 3, 500)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![enemy(-2, 1, 300), enemy(-3, 2, 700), enemy(-4, 3, 500)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);

    assert_eq!(
        TargetResolver::resolve(
            &TargetRequest {
                code: 207,
                raw: Vec::new(),
            },
            22_302_342,
            -3,
            &pool,
            &mut RoundDeterminism::default(),
        ),
        vec![11]
    );
}

#[test]
fn target_234_selects_the_sources_allied_assist_boss() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1)],
            assist_boss: Some(FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);

    assert_eq!(
        TargetResolver::resolve(
            &TargetRequest {
                code: 234,
                raw: Vec::new(),
            },
            116331900,
            10,
            &pool,
            &mut RoundDeterminism::default(),
        ),
        vec![-1]
    );
}

#[test]
fn status_targets_partition_allies_from_configured_buff_status() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity_at(10, 1),
                FightEntityInfo {
                    buffs: vec![buff(4010, 0)],
                    ..entity_at(11, 2)
                },
                entity_at(12, 3),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    let resolve = |code, determinism: &mut RoundDeterminism| {
        TargetResolver::resolve(
            &TargetRequest {
                code,
                raw: vec![BuffStatus::Control as i32],
            },
            1001,
            10,
            &pool,
            determinism,
        )
    };

    assert_eq!(resolve(249, &mut determinism), vec![11]);
    assert_eq!(resolve(250, &mut determinism), vec![10, 12]);
}

#[test]
fn target_132_selects_allies_with_the_configured_battle_tag() {
    init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        entity_type: Some(1),
        current_hp: Some(100),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, 3127),
                entity(11, 3134),
                FightEntityInfo {
                    destiny_stone: Some(308101),
                    destiny_rank: Some(1),
                    ..entity(12, 3081)
                },
                entity(13, 3081),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);

    assert_eq!(
        TargetResolver::resolve(
            &TargetRequest {
                code: 132,
                raw: vec![114],
            },
            1,
            10,
            &pool,
            &mut RoundDeterminism::default(),
        ),
        vec![10, 11, 12]
    );
}
