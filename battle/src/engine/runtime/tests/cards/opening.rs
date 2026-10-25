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
    let mut runtime = runtime(fight);

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
fn generated_opening_reaches_the_round_with_every_basic_card() {
    crate::test_support::init_config();
    let fight = Fight {
        battle_id: Some(7),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(1001),
                    position: Some(1),
                    current_hp: Some(100),
                    skill_group1: vec![101],
                    skill_group2: vec![201],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    model_id: Some(1002),
                    position: Some(2),
                    current_hp: Some(100),
                    skill_group1: vec![102],
                    skill_group2: vec![202],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(12),
                    model_id: Some(1003),
                    position: Some(3),
                    current_hp: Some(100),
                    skill_group1: vec![103],
                    skill_group2: vec![203],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = runtime(fight);

    let round = runtime.build_start_round_from_schedule().unwrap();

    assert_eq!(round.team_a_cards1.len(), 7);
    for skill_id in [101, 201, 102, 202, 103, 203] {
        assert!(
            round
                .team_a_cards1
                .iter()
                .any(|card| card.skill_id == Some(skill_id))
        );
    }
}

#[test]
fn rejected_opening_seed_does_not_retain_captured_draws() {
    crate::test_support::init_config();
    let fight = Fight {
        battle_id: Some(17),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(1001),
                position: Some(1),
                current_hp: Some(100),
                skill_group1: vec![101],
                skill_group2: vec![102],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(2001),
                position: Some(1),
                current_hp: Some(100),
                skill_group1: vec![201],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let player_card = |skill_id| sonettobuf::CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        ..Default::default()
    };
    let ai_card = sonettobuf::CardInfo {
        uid: Some(-1),
        skill_id: Some(201),
        ..Default::default()
    };
    let install_ai_skill = |runtime: &mut BattleRuntime| {
        runtime
            .catalog
            .insert(crate::engine::skill::effect::slot::ParsedSkillEffect {
                skill_id: 201,
                slots: Vec::new(),
            });
        runtime.catalog.insert_logic_target(201, 201);
    };
    let mut baseline = runtime(fight.clone());
    install_ai_skill(&mut baseline);
    let baseline_round = baseline.start_round().unwrap();
    let mut determinism = RoundDeterminism::with_seed(17);
    determinism.enqueue_opening_seed(
        vec![ai_card.clone(), ai_card],
        vec![player_card(101), player_card(102), player_card(101)],
        vec![player_card(102), player_card(102), player_card(102)],
        0,
    );
    let mut replay = runtime(fight);
    install_ai_skill(&mut replay);
    let replay_round = replay.start_round_with_determinism(determinism).unwrap();

    assert_eq!(replay_round.ai_use_cards, baseline_round.ai_use_cards);
    assert_eq!(replay_round.team_a_cards1, baseline_round.team_a_cards1);
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
    let mut runtime = runtime(fight);

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
fn version_seven_opening_deals_before_composition_and_hands_after() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(230556490),
                    position: Some(1),
                    current_hp: Some(100),
                    skill_group1: vec![31020111, 31020112, 31020113],
                    skill_group2: vec![31020121, 31020122, 31020123],
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
        uid: Some(230556490),
        skill_id: Some(skill_id),
        temp_card: Some(false),
        ..Default::default()
    };
    let opening = vec![
        card(31020121),
        card(31020111),
        card(31020121),
        card(31020111),
        card(31020111),
    ];
    let replacement = card(31020121);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_start_decks(Vec::new(), opening.clone());
    determinism.enqueue_card_draws(
        opening
            .iter()
            .cloned()
            .chain(std::iter::once(replacement))
            .collect(),
    );
    let mut runtime = runtime(fight);

    let round = runtime.start_round_with_determinism(determinism).unwrap();
    let push = runtime.card_info_push();
    let skills = |cards: &[CardInfo]| {
        cards
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>()
    };
    // The dealt snapshot keeps the cards as dealt; the hand shows them after
    // composition.
    assert_eq!(
        skills(&round.team_a_cards1),
        vec![31020121, 31020111, 31020121, 31020111, 31020111, 31020121]
    );
    assert_eq!(round.team_a_cards1, push.deal_card_group);
    assert_eq!(
        skills(&push.card_group),
        vec![31020121, 31020111, 31020121, 31020112, 31020121]
    );
    assert_eq!(push.card_group, runtime.managers.card.hand());
    fn contains_effect(steps: &[sonettobuf::FightStep], types: &[i32]) -> bool {
        steps
            .iter()
            .flat_map(|step| &step.act_effect)
            .any(|effect| {
                types.contains(&effect.effect_type.unwrap_or_default())
                    || effect
                        .fight_step
                        .as_ref()
                        .is_some_and(|nested| contains_effect(std::slice::from_ref(nested), types))
            })
    }
    assert!(!contains_effect(
        &round.fight_step,
        &[
            sonettobuf::effect_type_enum::EffectType::Addhandcard as i32,
            sonettobuf::effect_type_enum::EffectType::Cardspush as i32,
            sonettobuf::effect_type_enum::EffectType::Allocatecardenergy as i32,
        ]
    ));
    assert!(round.fight_step.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Cardscompose as i32)
        })
    }));
}

#[test]
fn version_six_opening_keeps_the_legacy_deal_snapshot_after_composition() {
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
    let mut runtime = runtime(fight);

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
