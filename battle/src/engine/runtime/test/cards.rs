use super::*;

#[test]
fn destination_start_schedule_builds_the_complete_round_wrapper() {
    crate::test_support::init_config();
    let fight = Fight {
        battle_id: Some(7),
        version: Some(6),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(1001),
                position: Some(1),
                current_hp: Some(100),
                skill_group1: vec![101],
                skill_group2: vec![201],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(fight);

    let round = runtime.build_start_round_from_schedule().unwrap();

    assert_eq!(round.cur_round, Some(1));
    assert_eq!(round.act_point, Some(1));
    assert_eq!(round.team_a_cards1.len(), 3);
    assert!(!round.fight_step.is_empty());
    assert_eq!(runtime.managers.card.hand().len(), 3);
    assert_eq!(
        round.ex_point_info,
        runtime.managers.ex_point_info(&runtime.fight)
    );
}

#[test]
fn opening_adds_one_ready_ultimate_outside_the_normal_hand() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3028),
                position: Some(1),
                current_hp: Some(100),
                ex_point: Some(5),
                ex_skill: Some(30280131),
                skill_group1: vec![30280111, 30280112, 30280113],
                skill_group2: vec![30280121, 30280122, 30280123],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(fight);

    let round = runtime.build_start_round_from_schedule().unwrap();

    assert_eq!(
        round
            .team_a_cards1
            .iter()
            .filter(|card| card.skill_id == Some(30280131))
            .count(),
        1
    );
    assert_eq!(
        runtime
            .managers
            .card
            .hand()
            .iter()
            .filter(|card| card.skill_id == Some(30280131))
            .count(),
        1
    );
}

#[test]
fn opening_push_keeps_composed_cards_and_refills_the_vacated_slot() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(6),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    position: Some(1),
                    current_hp: Some(100),
                    skill_group1: vec![100, 101, 102],
                    skill_group2: vec![200, 201, 202],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    position: Some(2),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let card = |skill_id| CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        temp_card: Some(false),
        ..Default::default()
    };
    let opening = vec![card(200), card(100), card(200), card(100), card(100)];
    let replacement = card(200);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_start_decks(Vec::new(), opening.clone());
    determinism.enqueue_card_draws(
        opening
            .iter()
            .cloned()
            .chain(std::iter::once(replacement))
            .collect(),
    );
    let mut runtime = BattleRuntime::new(fight);

    let round = runtime.start_round_with_determinism(determinism).unwrap();
    let push = runtime.card_info_push();
    let skills = |cards: &[CardInfo]| {
        cards
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        skills(&round.team_a_cards1),
        vec![200, 100, 200, 100, 100, 200]
    );
    assert_eq!(skills(&push.card_group), vec![200, 100, 200, 101, 200]);
    assert_eq!(
        skills(&push.deal_card_group),
        vec![200, 100, 200, 100, 100, 200]
    );
    assert_eq!(push.card_group, runtime.managers.card.hand());
    assert!(round.fight_step.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Cardscompose as i32)
        })
    }));
}

#[test]
fn teaching_card_opening_replaces_only_the_initial_random_draw() {
    crate::test_support::init_config();
    let fight = Fight {
        episode_id: Some(10002),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    model_id: Some(100102),
                    position: Some(1),
                    current_hp: Some(100),
                    skill_group1: vec![30250111, 30250112, 30250113],
                    skill_group2: vec![30250121, 30250122, 30250123],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    model_id: Some(100101),
                    position: Some(2),
                    current_hp: Some(100),
                    skill_group1: vec![30230111, 30230112, 30230113],
                    skill_group2: vec![30230121, 30230122, 30230123],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut determinism = RoundDeterminism::with_seed(9);
    determinism.enqueue_card_draws(vec![CardInfo {
        uid: Some(-2),
        skill_id: Some(30230121),
        ..Default::default()
    }]);
    let mut runtime = BattleRuntime::new(fight);

    let round = runtime.start_round_with_determinism(determinism).unwrap();

    assert!(runtime.determinism.has_queued_card_draw());
    assert_eq!(
        runtime
            .managers
            .card
            .refilled()
            .iter()
            .map(|card| (card.uid.unwrap(), card.skill_id.unwrap()))
            .collect::<Vec<_>>(),
        vec![(-2, 30230121), (-1, 30250121)]
    );
    assert_eq!(
        round
            .team_a_cards1
            .iter()
            .map(|card| (card.uid.unwrap(), card.skill_id.unwrap()))
            .collect::<Vec<_>>(),
        vec![
            (-1, 30250121),
            (-1, 30250121),
            (-1, 30250121),
            (-2, 30230111),
            (-2, 30230111),
            (-2, 30230121),
            (-1, 30250121),
        ]
    );
    assert_eq!(
        runtime
            .card_info_push()
            .card_group
            .iter()
            .map(|card| (card.uid.unwrap(), card.skill_id.unwrap()))
            .collect::<Vec<_>>(),
        vec![
            (-1, 30250122),
            (-1, 30250121),
            (-2, 30230112),
            (-2, 30230121),
            (-1, 30250121),
        ]
    );
}

#[test]
fn teaching_card_round_refill_replays_the_live_tutorial_operations() {
    crate::test_support::init_config();
    let fight = Fight {
        battle_id: Some(1002),
        episode_id: Some(10002),
        version: Some(7),
        max_round: Some(20),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    model_id: Some(100102),
                    position: Some(1),
                    current_hp: Some(10_000),
                    skill_group1: vec![30250111, 30250112, 30250113],
                    skill_group2: vec![30250121, 30250122, 30250123],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    model_id: Some(100101),
                    position: Some(2),
                    current_hp: Some(10_000),
                    skill_group1: vec![30230111, 30230112, 30230113],
                    skill_group2: vec![30230121, 30230122, 30230123],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-3),
                model_id: Some(100104),
                position: Some(1),
                current_hp: Some(100_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(fight);
    runtime
        .start_round_with_determinism(RoundDeterminism::with_seed(0x5eed))
        .unwrap();

    runtime
        .advance_round(sonettobuf::BeginRoundRequest {
            opers: vec![
                sonettobuf::BeginRoundOper {
                    oper_type: Some(1),
                    param1: Some(5),
                    param2: Some(2),
                    ..Default::default()
                },
                sonettobuf::BeginRoundOper {
                    oper_type: Some(2),
                    param1: Some(1),
                    to_id: Some(-3),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .unwrap();
    let skills = |cards: &[CardInfo]| {
        cards
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        skills(runtime.managers.card.refilled()),
        vec![30250111, 30230111, 30230111, 30250121]
    );
    assert_eq!(
        skills(runtime.managers.card.hand()),
        vec![30230112, 30230121, 30250111, 30230112, 30250121]
    );
}

#[test]
fn teaching_card_opening_composes_the_complete_configured_deal() {
    crate::test_support::init_config();
    let fight = Fight {
        episode_id: Some(10003),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(100109),
                position: Some(1),
                current_hp: Some(100),
                skill_group1: vec![30230111, 30230112, 30230113],
                skill_group2: vec![30230121, 30230122, 30230123],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(fight);

    let round = runtime.build_start_round_from_schedule().unwrap();

    assert_eq!(round.team_a_cards1.len(), 7);
    assert_eq!(
        runtime
            .card_info_push()
            .card_group
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![30230122, 30230112, 30230122, 30230111]
    );
}

#[test]
fn teaching_card_refill_follows_the_configured_draws_after_tutorial_plays() {
    crate::test_support::init_config();
    let fight = Fight {
        battle_id: Some(1001),
        episode_id: Some(10001),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    model_id: Some(100102),
                    position: Some(1),
                    current_hp: Some(100),
                    skill_group1: vec![30250111, 30250112, 30250113],
                    skill_group2: vec![30250121, 30250122, 30250123],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    model_id: Some(100101),
                    position: Some(2),
                    current_hp: Some(100),
                    skill_group1: vec![30230111, 30230112, 30230113],
                    skill_group2: vec![30230121, 30230122, 30230123],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(fight);
    runtime
        .start_round_with_determinism(RoundDeterminism::with_seed(0x5eed))
        .unwrap();
    let skills = |cards: &[CardInfo]| {
        cards
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>()
    };

    assert_eq!(
        skills(runtime.managers.card.hand()),
        vec![30230111, 30230121, 30250111, 30250121, 30230111]
    );
    for hand_index in [4, 3] {
        runtime
            .managers
            .execute_card(crate::engine::manager::card::CardCommand::Play(
                crate::engine::manager::card::CardPlay {
                    origin: crate::engine::manager::card::CARD_PLAY_ORIGIN,
                    hand_index,
                    target_uid: None,
                    chosen_skill_id: None,
                    choice: None,
                    recorded_skill: None,
                },
            ))
            .unwrap();
    }
    let pool = crate::engine::skill::target::TargetPool::from_fight(&runtime.fight);
    schedule::run_round_refill(
        &mut runtime.managers,
        &pool,
        &runtime.catalog,
        &mut runtime.determinism,
        crate::engine::skill::target::TargetContext {
            battle_id: 1001,
            current_round: 1,
            ..Default::default()
        },
        5,
        1,
    )
    .unwrap();

    assert_eq!(
        skills(runtime.managers.card.hand()),
        vec![30230111, 30230121, 30250111, 30230121, 30250111]
    );
}

#[test]
fn teaching_card_without_scripted_refills_preserves_its_composed_hand_size() {
    crate::test_support::init_config();
    let fight = Fight {
        battle_id: Some(11011),
        episode_id: Some(10101),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3028),
                position: Some(1),
                current_hp: Some(1_000),
                skill_group1: vec![30280111, 30280112, 30280113],
                skill_group2: vec![30280121, 30280122, 30280123],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(fight);
    runtime
        .start_round_with_determinism(RoundDeterminism::with_seed(0x5eed))
        .unwrap();
    let opening_size = runtime.managers.card.normal_hand_len();
    assert_eq!(opening_size, 4);

    runtime
        .managers
        .execute_card(crate::engine::manager::card::CardCommand::Play(
            crate::engine::manager::card::CardPlay {
                origin: crate::engine::manager::card::CARD_PLAY_ORIGIN,
                hand_index: 0,
                target_uid: None,
                chosen_skill_id: None,
                choice: None,
                recorded_skill: None,
            },
        ))
        .unwrap();
    let pool = crate::engine::skill::target::TargetPool::from_fight(&runtime.fight);
    let hand_size = crate::engine::mechanic::card::CardMechanic.normal_hand_limit(
        crate::engine::manager::card::start::hand_size(&runtime.fight),
        &runtime.managers,
        &pool,
    );
    schedule::run_round_refill(
        &mut runtime.managers,
        &pool,
        &runtime.catalog,
        &mut runtime.determinism,
        Default::default(),
        hand_size,
        1,
    )
    .unwrap();

    assert_eq!(runtime.managers.card.normal_hand_len(), opening_size);
    assert!(!runtime.managers.card.refilled().is_empty());
    assert_eq!(runtime.managers.card.deck_num(), 16);
}

#[test]
fn next_round_snapshot_keeps_card_not_cal_size_ultimate() {
    crate::test_support::init_config();
    let ultimate = 31390131;
    let fight = Fight {
        cur_round: Some(1),
        version: Some(6),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3139),
                position: Some(1),
                current_hp: Some(100),
                ex_point: Some(5),
                ex_skill: Some(ultimate),
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31390181),
                    from_uid: Some(10),
                    ..Default::default()
                }],
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
    let card = |skill_id| CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(fight);
    runtime
        .managers
        .execute_card(crate::engine::manager::card::CardCommand::Setup(
            CardSetup {
                hand: vec![card(100), card(200), card(300)],
                draw_pile: Vec::new(),
                deck_num: 0,
            },
        ))
        .unwrap();

    let first = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();
    assert!(
        first
            .team_a_cards1
            .iter()
            .any(|card| { card.uid == Some(10) && card.skill_id == Some(ultimate) })
    );
    assert!(
        runtime
            .managers
            .card
            .team_cards()
            .iter()
            .any(|card| { card.uid == Some(10) && card.skill_id == Some(ultimate) })
    );
    assert!(first.next_round_begin_step.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect
                .card_info_list
                .iter()
                .any(|card| card.uid == Some(10) && card.skill_id == Some(ultimate))
        })
    }));

    let second = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();
    assert!(
        second
            .team_a_cards1
            .iter()
            .any(|card| { card.uid == Some(10) && card.skill_id == Some(ultimate) })
    );
    assert!(
        second
            .before_cards1
            .iter()
            .all(|card| card.skill_id != Some(ultimate))
    );
    assert!(second.next_round_begin_step.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect
                .card_info_list
                .iter()
                .filter(|card| card.uid == Some(10) && card.skill_id == Some(ultimate))
                .count()
                == 1
        })
    }));
}

#[test]
fn round_start_generated_card_is_committed_after_the_before_cards_snapshot() {
    crate::test_support::init_config();
    let generated_skill = 312451035;
    let fight = Fight {
        cur_round: Some(1),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(10),
                ex_point_type: Some(3),
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(20),
                    buff_id: Some(312451407),
                    from_uid: Some(10),
                    ..Default::default()
                }],
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
    let card = |skill_id| CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(fight);
    runtime
        .managers
        .execute_card(crate::engine::manager::card::CardCommand::Setup(
            CardSetup {
                hand: vec![card(100), card(200), card(300)],
                draw_pile: Vec::new(),
                deck_num: 0,
            },
        ))
        .unwrap();

    let round = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();

    assert!(
        round
            .before_cards1
            .iter()
            .all(|card| card.skill_id != Some(generated_skill))
    );
    assert!(
        runtime
            .managers
            .card
            .hand()
            .iter()
            .any(|card| card.skill_id == Some(generated_skill))
    );
    assert!(round.next_round_begin_step.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect
                .card_info_list
                .iter()
                .any(|card| card.skill_id == Some(generated_skill))
                || effect.fight_step.as_ref().is_some_and(|nested| {
                    nested.act_effect.iter().any(|effect| {
                        effect
                            .card_info_list
                            .iter()
                            .any(|card| card.skill_id == Some(generated_skill))
                    })
                })
        })
    }));
}

#[test]
fn lorentz_team_ultimate_spends_beryl_moxie_and_is_not_regenerated() {
    crate::test_support::init_config();
    let entity =
        |uid, model_id, ex_point, ex_skill, passive_skill, skill1, skill2| FightEntityInfo {
            uid: Some(uid),
            model_id: Some(model_id),
            position: Some(uid as i32),
            team_type: Some(1),
            current_hp: Some(100_000),
            attr: Some(sonettobuf::HeroAttribute {
                hp: Some(100_000),
                attack: Some(1_000),
                ..Default::default()
            }),
            ex_point: Some(ex_point),
            ex_skill: Some(ex_skill),
            passive_skill,
            skill_group1: skill1,
            skill_group2: skill2,
            ..Default::default()
        };
    let passives = |hero_id, destiny| {
        crate::engine::entity::passive::Passive::for_config(hero_id, None, destiny)
            .into_iter()
            .map(|passive| passive.skill_id)
            .collect::<Vec<_>>()
    };
    let mut isolde = entity(
        10,
        3081,
        0,
        30810131,
        passives(3081, Some((308101, 4))),
        vec![30810111, 30810112, 30810113],
        vec![30810121, 30810122, 30810123],
    );
    isolde.destiny_stone = Some(308101);
    isolde.destiny_rank = Some(4);
    let fight = Fight {
        battle_id: Some(6),
        version: Some(6),
        attacker: Some(FightTeam {
            entitys: vec![
                isolde,
                entity(
                    11,
                    3127,
                    0,
                    31270131,
                    passives(3127, None),
                    vec![31270111, 31270112, 31270113],
                    vec![31270121, 31270122, 31270123],
                ),
                entity(
                    12,
                    3134,
                    5,
                    31345131,
                    passives(3134, None),
                    vec![31345111, 31345112, 31345113],
                    vec![31344121, 31344122, 31344123],
                ),
                entity(
                    13,
                    3139,
                    0,
                    31390131,
                    passives(3139, None),
                    vec![31390111, 31390112, 31390113],
                    vec![31390121, 31390122, 31390123],
                ),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(1000),
                team_type: Some(2),
                current_hp: Some(1_000_000),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(1_000_000),
                    defense: Some(1_000),
                    mdefense: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let normal_cards = [
        (10, 30810111),
        (11, 31270111),
        (12, 31345111),
        (13, 31390111),
        (10, 30810121),
        (11, 31270121),
        (12, 31344121),
        (13, 31390121),
        (10, 30810111),
    ]
    .into_iter()
    .map(|(uid, skill_id)| CardInfo {
        uid: Some(uid),
        skill_id: Some(skill_id),
        ..Default::default()
    })
    .collect::<Vec<_>>();
    let mut determinism = RoundDeterminism::with_seed(6);
    determinism.enqueue_start_decks(Vec::new(), normal_cards.clone());
    determinism.enqueue_card_draws(normal_cards);
    let mut runtime = BattleRuntime::new(fight);
    runtime.start_round_with_determinism(determinism).unwrap();
    assert!(
        runtime
            .use_cloth_skill(UseClothSkillRequest {
                from_id: Some(12),
                to_id: Some(220),
                r#type: Some(ClothSkillType::SelectCrystal as i32),
                ..Default::default()
            })
            .is_some()
    );
    runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();
    let beryl_precast_index = runtime
        .managers
        .card
        .hand()
        .iter()
        .position(|card| card.uid == Some(12) && card.temp_card.unwrap_or_default())
        .unwrap();
    let beryl_ultimate_index_after_precast = runtime.managers.card.hand().len() - 1
        + runtime
            .managers
            .card
            .team_cards()
            .iter()
            .position(|card| card.uid == Some(12) && card.skill_id == Some(31345131))
            .unwrap();

    let round = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest {
            opers: vec![
                BeginRoundOper {
                    oper_type: Some(2),
                    param1: Some(beryl_precast_index as i32 + 1),
                    to_id: Some(-1),
                    ..Default::default()
                },
                BeginRoundOper {
                    oper_type: Some(2),
                    param1: Some(beryl_ultimate_index_after_precast as i32 + 1),
                    to_id: Some(-1),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .unwrap();

    // Beryl's eligibility marker is removed when she casts her Ultimate, so
    // Lorentz's team passive does not refund her spent Moxie. Lorentz did not
    // cast an Ultimate, so her +1 team grant and additional +2 self grant both
    // apply after the +2 she gained on entering battle.
    assert_eq!(runtime.managers.ex_point.get(12), 0);
    assert_eq!(runtime.managers.ex_point.get(13), 5);
    assert!(
        runtime
            .managers
            .card
            .team_cards()
            .iter()
            .all(|card| { card.uid != Some(12) || card.skill_id != Some(31345131) })
    );
    assert!(round.fight_step.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32)
                && effect.target_id == Some(12)
                && effect.effect_num == Some(-5)
        })
    }));
}
