use super::*;

#[test]
fn resolves_buff_type_feature_and_monster_label_targets() {
    init_config();

    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                model_id: Some(3132),
                ..entity_at(10, 1)
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                enemy_with_buff(-10, 1, 80, buff(31320113, 31320113)),
                enemy_with_buff(-11, 2, 30, buff(30840111, 0)),
                enemy_with_buff(-12, 3, 60, buff(530000413, 0)),
                enemy_with_buff(-13, 4, 70, buff(222, 0)),
                enemy_with_buff(-14, 5, 70, buff(30970111, 0)),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();

    assert_eq!(resolve_code(245, 10, &pool, &mut determinism), vec![-10]);
    assert_eq!(
        TargetResolver::resolve(
            &TargetRequest {
                code: 247,
                raw: vec![31320113],
            },
            1001,
            10,
            &pool,
            &mut determinism,
        ),
        vec![-10]
    );
    assert_eq!(
        resolve_code_with_context(
            307,
            10,
            &pool,
            &mut determinism,
            TargetContext {
                runtime_target_uid: -13,
                logic_target: 0,
                extra_skill_kind: 0,
                ..Default::default()
            },
        ),
        vec![-12]
    );
    assert_eq!(resolve_code(230, 10, &pool, &mut determinism), vec![-14]);
    assert_eq!(resolve_code(4101, 10, &pool, &mut determinism), vec![-11]);
}

#[test]
fn alternating_monster_targets_keep_their_exact_label_rules() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    model_id: Some(900016101),
                    ..entity_at(-2, 1)
                },
                FightEntityInfo {
                    model_id: Some(900016102),
                    ..entity_at(-3, 2)
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();

    assert_eq!(resolve_code(1007, -2, &pool, &mut determinism), vec![-2]);
    assert_eq!(resolve_code(1008, -2, &pool, &mut determinism), vec![-3]);
    assert_eq!(targets_enemy(1007), Some(false));
    assert_eq!(targets_enemy(1008), Some(false));
}

#[test]
fn gorgon_tentacle_targets_keep_their_exact_label_rules() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    model_id: Some(150401),
                    ..entity_at(-1, 1)
                },
                FightEntityInfo {
                    model_id: Some(150402),
                    ..entity_at(-2, 2)
                },
                FightEntityInfo {
                    model_id: Some(150403),
                    ..entity_at(-3, 3)
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();

    assert_eq!(resolve_code(1009, -1, &pool, &mut determinism), vec![-2]);
    assert_eq!(resolve_code(1010, -1, &pool, &mut determinism), vec![-3]);
    assert_eq!(targets_enemy(1009), Some(false));
    assert_eq!(targets_enemy(1010), Some(false));
}

#[test]
fn qualified_enemy_targets_do_not_substitute_an_unrelated_enemy() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity_at(-1, 1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();

    assert!(resolve_code(245, 10, &pool, &mut determinism).is_empty());
    assert!(resolve_code(307, 10, &pool, &mut determinism).is_empty());
}

#[test]
fn shell_target_235_selects_the_enemy_with_most_configured_shell() {
    init_config();
    let shell = |uid, layer| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
        team_type: Some(2),
        buffs: vec![BuffInfo {
            uid: Some(100_000 - uid),
            buff_id: Some(31090112),
            layer: Some(layer),
            ..Default::default()
        }],
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                team_type: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(52),
                    buff_id: Some(31090111),
                    layer: Some(8),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![shell(-1, 2), shell(-2, 5), shell(-3, 3)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);

    assert_eq!(
        TargetResolver::resolve_with_managers_and_context(
            &TargetRequest {
                code: 235,
                raw: Vec::new(),
            },
            31090174,
            10,
            &pool,
            &mut RoundDeterminism::default(),
            Some(&managers),
            TargetContext::default(),
        ),
        vec![-2]
    );
}

#[test]
fn target_244_selects_the_enemy_with_most_queued_attack_incantations() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity_at(10, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity_at(-1, 1), entity_at(-2, 2), entity_at(-3, 3)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(crate::engine::manager::card::CardCommand::SetAiQueue(
            crate::engine::manager::card::CardSetAiQueue {
                origin: crate::engine::skill::rule::CommandOrigin {
                    domain: crate::engine::skill::rule::RuleDomain::Lifecycle,
                    key: crate::engine::skill::rule::DefinitionKey::new(0, "TestAiQueue"),
                },
                cards: vec![
                    queued_card(-1, 31230161),
                    queued_card(-2, 31230161),
                    queued_card(-2, 312301611),
                    queued_card(-3, 31230151),
                ],
            },
        ))
        .unwrap();

    assert_eq!(
        TargetResolver::resolve_with_managers_and_context(
            &TargetRequest {
                code: 244,
                raw: Vec::new(),
            },
            31230101,
            10,
            &pool,
            &mut RoundDeterminism::default(),
            Some(&managers),
            TargetContext::default(),
        ),
        vec![-2]
    );
}
