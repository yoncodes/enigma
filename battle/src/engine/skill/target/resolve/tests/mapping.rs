use super::*;

#[test]
fn unmapped_target_codes_are_detectable_before_capture_override() {
    assert!(is_mapped_target_code(
        crate::engine::skill::target::request::SOURCE_TARGET_CODE
    ));
    assert!(is_mapped_target_code(128));
    assert!(is_mapped_target_code(208));
    assert!(is_mapped_target_code(3102));
    assert!(!is_mapped_target_code(999));
}

#[test]
fn afflatus_target_codes_select_matching_allies() {
    let entity = |uid, position, career| FightEntityInfo {
        career: Some(career),
        ..entity_at(uid, position)
    };
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 2), entity(11, 2, 2), entity(12, 3, 3)],
            ..Default::default()
        }),
        ..Default::default()
    });

    assert_eq!(
        resolve_code(3102, 10, &pool, &mut RoundDeterminism::default()),
        vec![10, 11]
    );
    assert_eq!(
        resolve_code(3103, 10, &pool, &mut RoundDeterminism::default()),
        vec![12]
    );
}

#[test]
fn overlapping_enemy_codes_keep_their_exact_rules() {
    assert_eq!(target_rule(210), Some(TargetRule::LowestHpPercentageEnemy));
    assert_eq!(target_rule(221), Some(TargetRule::PriorityBossEnemy));
    assert_eq!(target_rule(231), Some(TargetRule::BurnPriorityEnemy));
    assert_eq!(target_rule(233), Some(TargetRule::Runtime));
    assert_eq!(target_rule(232), Some(TargetRule::EnemyPosition(7)));
    assert_eq!(target_rule(234), Some(TargetRule::AssistBoss));
}

#[test]
fn target_309_is_the_selected_bound_ally() {
    assert_eq!(target_rule(309), Some(TargetRule::BoundAlly));
    assert_eq!(targets_enemy(309), Some(false));
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
fn captured_ally_slot_targets_select_exact_positions() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity_at(10, 1),
                entity_at(11, 2),
                entity_at(12, 3),
                entity_at(13, 4),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);

    for (code, uid) in [(124, 10), (125, 11), (127, 13)] {
        assert_eq!(
            resolve_code(code, 0, &pool, &mut RoundDeterminism::default()),
            vec![uid]
        );
    }
    assert!(target_rule(126).is_none());
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
fn hp_percentage_targets_use_widened_ratio_arithmetic() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                entity_stats(-2, 1, 100_000_000, 100_000_000, 0),
                entity_stats(-3, 2, 90_000_000, 100_000_000, 0),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);

    assert_eq!(
        resolve_code(210, 10, &pool, &mut RoundDeterminism::default()),
        vec![-3]
    );
    assert_eq!(
        resolve_code(208, 10, &pool, &mut RoundDeterminism::default()),
        vec![-2]
    );
}

#[test]
fn highest_hp_enemy_waits_for_reserve_entry() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity_stats(-2, 1, 40, 100, 0)],
            sub_entitys: vec![entity_stats(-3, -1, 100, 100, 0)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let select = |managers: &BattleManagers| {
        resolve_code(
            208,
            10,
            &pool.runtime_view(managers),
            &mut RoundDeterminism::default(),
        )
    };

    assert_eq!(select(&managers), vec![-2]);
    managers.hp.lose(-2, 40, 0);
    assert!(select(&managers).is_empty());
    assert_eq!(managers.entity.promote_reserves(&managers.hp).len(), 1);
    assert_eq!(select(&managers), vec![-3]);
}

#[test]
fn enemy_rules_skip_waiting_reserves() {
    init_config();
    let enemy = |uid, position, current_hp, attack| FightEntityInfo {
        attr: Some(HeroAttribute {
            hp: Some(100),
            attack: Some(attack),
            ..Default::default()
        }),
        ..entity_stats(uid, position, current_hp, 100, 0)
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![enemy(-2, 1, 80, 100), enemy(-3, 2, 60, 200)],
            // Would win every ranking below if it were on the field.
            sub_entitys: vec![enemy(-4, -1, 10, 900)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let select = |code| resolve_code(code, 10, &pool, &mut RoundDeterminism::default());

    assert_eq!(select(302), vec![-2, -3]);
    assert_eq!(select(207), vec![-3]);
    assert_eq!(select(208), vec![-2]);
    assert_eq!(select(210), vec![-3]);
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
            sub_entitys: vec![
                FightEntityInfo {
                    buffs: vec![buff(4010, 0)],
                    ..entity_at(13, -1)
                },
                entity_at(14, -1),
            ],
            sp_entitys: vec![
                FightEntityInfo {
                    buffs: vec![buff(4010, 0)],
                    ..entity_at(15, 5)
                },
                entity_at(16, 6),
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

    assert_eq!(resolve(249, &mut determinism), vec![11, 15]);
    assert_eq!(resolve(250, &mut determinism), vec![10, 12, 16]);
}

#[test]
fn target_132_selects_direct_and_mapped_battle_tags() {
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
                entity(14, 3146),
                entity(15, 3145),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);

    let resolve = |tag| {
        TargetResolver::resolve(
            &TargetRequest {
                code: 132,
                raw: vec![tag],
            },
            1,
            10,
            &pool,
            &mut RoundDeterminism::default(),
        )
    };
    assert_eq!(resolve(114), vec![10, 11, 12, 14]);
    assert_eq!(resolve(131), vec![14, 15]);
    assert_eq!(resolve(102), vec![12, 14]);
    assert_eq!(resolve(117), vec![10, 13, 15]);
}

#[test]
fn target_2001_selects_enemies_carrying_monster_label_one() {
    init_config();
    let marked = enemy_with_buff(20, 2, 100, buff(11410111, 11410111));
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1), marked],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity_at(-1, 1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);

    assert_eq!(
        resolve_code(2001, -1, &pool, &mut RoundDeterminism::default()),
        vec![20]
    );
}

#[test]
fn ally_pickers_skip_waiting_reserves() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity_stats(10, 1, 60, 100, 3),
                entity_stats(20, 2, 80, 100, 2),
            ],
            sub_entitys: vec![entity_stats(30, -1, 10, 100, 0)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity_at(-1, 1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let select = |code| resolve_code(code, 20, &pool, &mut RoundDeterminism::default());

    assert_eq!(select(107), vec![10]);
    assert_eq!(select(111), vec![20]);
}
