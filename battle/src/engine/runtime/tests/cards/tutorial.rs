use super::*;

#[test]
fn tutorial_without_scripted_cards_uses_the_normal_opening() {
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
    let mut runtime = runtime(fight);

    let round = runtime.start_round_with_determinism(determinism).unwrap();

    assert!(!runtime.determinism.has_queued_card_draw());
    let push = runtime.card_info_push();
    // Captured episode 10002 deals 7 and composes to a 5-card hand.
    assert_eq!(round.team_a_cards1.len(), 7);
    assert!(
        round
            .team_a_cards1
            .iter()
            .all(|card| card.uid == Some(-1) || card.uid == Some(-2))
    );
    assert_eq!(round.team_a_cards1, push.deal_card_group);
    assert_eq!(push.card_group.len(), 5);
}

#[test]
fn tutorial_without_scripted_refill_uses_the_normal_round_refill() {
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
    let mut runtime = runtime(fight);
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
    assert!(!runtime.managers.card.refilled().is_empty());
    assert_eq!(runtime.managers.card.normal_hand_len(), 5);
}

#[test]
fn tutorial_without_scripted_opening_uses_the_normal_hand_size() {
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
    let mut runtime = runtime(fight);

    let round = runtime.start_round().unwrap();

    let push = runtime.card_info_push();
    // Captured episode 10003 deals 7 and composes to a 4-card hand.
    assert_eq!(round.team_a_cards1.len(), 7);
    assert_eq!(round.team_a_cards1, push.deal_card_group);
    assert_eq!(push.card_group.len(), 4);
}

#[test]
fn tutorial_without_scripted_draws_refills_normally_after_plays() {
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
    let mut runtime = runtime(fight);
    runtime
        .start_round_with_determinism(RoundDeterminism::with_seed(0x5eed))
        .unwrap();
    assert_eq!(runtime.managers.card.normal_hand_len(), 5);
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

    assert_eq!(runtime.managers.card.normal_hand_len(), 5);
    assert!(!runtime.managers.card.refilled().is_empty());
}

#[test]
fn tutorial_without_scripted_refills_preserves_the_normal_hand_size() {
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
    let mut runtime = runtime(fight);
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
}
