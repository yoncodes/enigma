use sonettobuf::{Fight, FightEntityInfo, FightTeam};

use super::*;

#[test]
fn builds_start_decks_from_fight_entities() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, 1001, 2, &[101], &[201]),
                entity(11, 1002, 1, &[102], &[202]),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2001, 1, &[301], &[401])],
            ..Default::default()
        }),
        ..Default::default()
    };

    let mut ex_point = crate::engine::manager::ex_point::ExPointManager::default();
    ex_point.seed(&fight);
    let mut eureka = crate::engine::manager::eureka::EurekaManager::default();
    eureka.seed(&fight);
    let (ai, player) = start_decks_from_fight(
        crate::test_support::game_data(),
        &fight,
        &ex_point,
        &eureka,
        0,
        7,
        None,
    );
    let managers = crate::engine::manager::BattleManagers::seeded(&fight);
    let mut skill_catalog = crate::engine::skill::effect::SkillEffectCatalog::default();
    skill_catalog.insert(crate::engine::skill::effect::slot::ParsedSkillEffect {
        skill_id: 301,
        slots: Vec::new(),
    });
    skill_catalog.insert_logic_target(301, 201);
    let configured = configured_start_decks(
        &fight,
        &managers,
        &skill_catalog,
        &crate::engine::runtime::determinism::RoundDeterminism::default(),
        0,
        7,
        None,
    )
    .unwrap();
    assert_eq!(
        (configured.ai, configured.player),
        (ai.clone(), player.clone())
    );
    assert!(!configured.used_capture);

    assert_eq!(player.len(), 5);
    assert!(player.iter().any(|card| card.uid == Some(10)));
    assert!(player.iter().any(|card| card.uid == Some(11)));
    assert_eq!(ai.len(), 1);
    assert_eq!(ai[0].uid, Some(-1));
    assert_eq!(ai[0].skill_id, Some(301));
    assert!(matches!(ai[0].target_uid, Some(10 | 11)));
}

#[test]
fn generated_basic_coverage_is_versioned_and_does_not_require_device_owners() {
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, 1001, 1, &[101], &[201]),
                entity(11, 1002, 2, &[102], &[202]),
                entity(12, 1003, 3, &[103], &[203]),
                entity(13, 1004, 4, &[], &[]),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let card = |uid, skill_id| {
        card_for(
            fight
                .attacker
                .as_ref()
                .unwrap()
                .entitys
                .iter()
                .find(|entity| entity.uid == Some(uid))
                .unwrap(),
            Some(skill_id),
        )
        .unwrap()
    };
    let original = vec![
        card(10, 101),
        card(11, 102),
        card(12, 103),
        card(10, 201),
        card(11, 202),
        card(13, 301),
        card(13, 301),
        card(13, 301),
    ];
    let mut covered = original.clone();
    preserve_generated_basic_coverage(&fight, &mut covered);

    for skill_id in [101, 201, 102, 202, 103, 203] {
        assert!(covered.iter().any(|card| card.skill_id == Some(skill_id)));
    }
    assert_eq!(
        covered.iter().filter(|card| card.uid == Some(13)).count(),
        2
    );

    let mut legacy = original.clone();
    let mut legacy_fight = fight.clone();
    legacy_fight.version = Some(6);
    preserve_generated_basic_coverage(&legacy_fight, &mut legacy);
    assert_eq!(legacy, original);

    let mut devices = vec![card(13, 301), card(13, 301)];
    let device_fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![entity(13, 1004, 1, &[], &[])],
            ..Default::default()
        }),
        ..Default::default()
    };
    preserve_generated_basic_coverage(&device_fight, &mut devices);
    assert_eq!(devices, vec![card(13, 301), card(13, 301)]);
}

#[test]
fn version_seven_normal_hand_has_a_four_card_minimum_and_eight_card_cap() {
    let fight = |characters, version| Fight {
        version: Some(version),
        attacker: Some(FightTeam {
            entitys: (0..characters)
                .map(|index| {
                    entity(
                        index + 1,
                        1000 + index as i32,
                        index as i32 + 1,
                        &[101],
                        &[201],
                    )
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };

    assert_eq!(hand_size(&fight(0, 7)), 0);
    assert_eq!(hand_size(&fight(1, 6)), 3);
    assert_eq!(hand_size(&fight(1, 7)), 4);
    assert_eq!(hand_size(&fight(2, 7)), 5);
    assert_eq!(hand_size(&fight(3, 7)), 7);
    assert_eq!(hand_size(&fight(4, 7)), 8);
}

#[test]
fn captured_start_decks_accept_two_configured_ai_cards_for_one_enemy() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(12, 1002, 1, &[202], &[203])],
            sp_entitys: vec![entity(13, 3002, 1, &[], &[])],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-2, 2002, 1, &[302], &[303])],
            ..Default::default()
        }),
        ..Default::default()
    };
    let captured_player = CardInfo {
        uid: Some(12),
        skill_id: Some(202),
        card_effect: Some(999),
        ..Default::default()
    };
    let captured = (
        vec![
            CardInfo {
                uid: Some(-2),
                skill_id: Some(302),
                card_effect: Some(999),
                target_uid: Some(13),
                ..Default::default()
            },
            CardInfo {
                uid: Some(-2),
                skill_id: Some(303),
                target_uid: Some(999),
                ..Default::default()
            },
        ],
        vec![captured_player; hand_size(&fight)],
    );

    let mut ex_point = crate::engine::manager::ex_point::ExPointManager::default();
    ex_point.seed(&fight);
    let mut eureka = crate::engine::manager::eureka::EurekaManager::default();
    eureka.seed(&fight);
    let (ai, player) = start_decks_from_fight(
        crate::test_support::game_data(),
        &fight,
        &ex_point,
        &eureka,
        1,
        0,
        Some(captured),
    );

    assert_eq!(ai[0].skill_id, Some(302));
    assert_eq!(ai[0].card_effect, None);
    assert_eq!(ai[0].target_uid, Some(13));
    assert_eq!(ai[1].skill_id, Some(303));
    assert_eq!(ai[1].target_uid, Some(0));
    assert_eq!(ai.len(), 2);
    assert_eq!(player[0].skill_id, Some(202));
    assert_eq!(player[0].card_effect, None);
    assert_eq!(player.len(), hand_size(&fight));
}

#[test]
fn captured_opening_reserves_normal_ultimate_slots_without_padding_the_seed() {
    let mut attacker = entity(12, 1002, 1, &[202], &[203]);
    attacker.ex_skill = Some(900);
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![attacker],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-2, 2002, 1, &[302], &[303])],
            ..Default::default()
        }),
        ..Default::default()
    };
    let player_card = CardInfo {
        uid: Some(12),
        skill_id: Some(202),
        ..Default::default()
    };

    let configured = configured_start_decks(
        &fight,
        &crate::engine::manager::BattleManagers::seeded(&fight),
        &crate::engine::skill::effect::SkillEffectCatalog::default(),
        &crate::engine::runtime::determinism::RoundDeterminism::default(),
        0,
        0,
        Some(CapturedDeckSeed::Opening {
            ai: vec![CardInfo {
                uid: Some(-2),
                skill_id: Some(302),
                ..Default::default()
            }],
            player: vec![player_card; hand_size(&fight) - 1],
            reserved_ultimate_slots: 1,
        }),
    )
    .unwrap();

    assert!(configured.used_capture);
    assert_eq!(configured.player.len(), hand_size(&fight) - 1);
    assert!(
        configured
            .player
            .iter()
            .all(|card| card.skill_id == Some(202))
    );
}

#[test]
fn captured_player_opening_is_independent_from_the_ai_snapshot() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(12, 1002, 1, &[202], &[203])],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-2, 2002, 1, &[302], &[303])],
            ..Default::default()
        }),
        ..Default::default()
    };
    let player = CardInfo {
        uid: Some(12),
        skill_id: Some(202),
        ..Default::default()
    };

    let managers = crate::engine::manager::BattleManagers::seeded(&fight);
    let mut skill_catalog = crate::engine::skill::effect::SkillEffectCatalog::default();
    for skill_id in [302, 303] {
        skill_catalog.insert(crate::engine::skill::effect::slot::ParsedSkillEffect {
            skill_id,
            slots: Vec::new(),
        });
        skill_catalog.insert_logic_target(skill_id, 201);
    }
    let configured = configured_start_decks(
        &fight,
        &managers,
        &skill_catalog,
        &crate::engine::runtime::determinism::RoundDeterminism::default(),
        0,
        0,
        Some(CapturedDeckSeed::Opening {
            ai: vec![
                CardInfo {
                    uid: Some(-2),
                    skill_id: Some(302),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(-2),
                    skill_id: Some(303),
                    ..Default::default()
                },
            ],
            player: vec![player.clone(); hand_size(&fight)],
            reserved_ultimate_slots: 0,
        }),
    )
    .unwrap();

    assert!(configured.used_capture);
    assert!(!configured.used_ai_capture);
    assert_eq!(configured.ai.len(), 1);
    assert_eq!(configured.ai[0].target_uid, Some(12));
    assert!(
        configured
            .player
            .iter()
            .all(|card| card.skill_id == Some(202))
    );
}

#[test]
fn captured_next_ai_snapshot_validates_without_a_player_hand() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(12, 1002, 1, &[202], &[203])],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-2, 2002, 1, &[302], &[303])],
            ..Default::default()
        }),
        ..Default::default()
    };
    let configured = configured_start_decks(
        &fight,
        &crate::engine::manager::BattleManagers::seeded(&fight),
        &crate::engine::skill::effect::SkillEffectCatalog::default(),
        &crate::engine::runtime::determinism::RoundDeterminism::default(),
        1,
        0,
        Some(CapturedDeckSeed::NextAi(vec![
            CardInfo {
                uid: Some(-2),
                skill_id: Some(302),
                ..Default::default()
            },
            CardInfo {
                uid: Some(-2),
                skill_id: Some(303),
                ..Default::default()
            },
        ])),
    )
    .unwrap();

    assert_eq!(
        configured
            .ai
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![302, 303]
    );
    assert!(configured.player.is_empty());
    assert!(configured.used_capture);
}

#[test]
fn captured_next_ai_snapshot_rejects_a_replaced_target() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1002, 1, &[202], &[203])],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                entity(-1, 116330001, 1, &[116330021], &[]),
                FightEntityInfo {
                    current_hp: Some(0),
                    ..entity(-2, 116330100, 2, &[116330112], &[])
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);
    managers
        .execute_entity(crate::engine::manager::entity::EntityCommand {
            origin: crate::engine::skill::rule::CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Behavior,
                key: crate::engine::skill::rule::DefinitionKey::new(60008, "Summon"),
            },
            source_uid: -1,
            target_uid: -1,
            operation: crate::engine::manager::entity::EntityOperation::SummonCombatant {
                model_id: 116330100,
                position: 2,
            },
        })
        .unwrap();

    let configured = configured_start_decks(
        &fight,
        &managers,
        &crate::engine::skill::effect::SkillEffectCatalog::default(),
        &crate::engine::runtime::determinism::RoundDeterminism::default(),
        0,
        0,
        Some(CapturedDeckSeed::NextAi(vec![
            CardInfo {
                uid: Some(-1),
                skill_id: Some(116330021),
                ..Default::default()
            },
            CardInfo {
                uid: Some(-3),
                skill_id: Some(116330112),
                target_uid: Some(-2),
                ..Default::default()
            },
        ])),
    )
    .unwrap();

    assert!(configured.used_capture);
    assert_eq!(
        configured
            .ai
            .iter()
            .filter_map(|card| card.uid)
            .collect::<Vec<_>>(),
        vec![-1, -3]
    );
    assert_eq!(configured.ai[1].target_uid, Some(0));
}

#[test]
fn captured_player_opening_rejects_count_or_identity_mismatch() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(12, 1002, 1, &[202], &[203])],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-2, 2002, 1, &[302], &[303])],
            ..Default::default()
        }),
        ..Default::default()
    };
    let ai_card = |skill_id| CardInfo {
        uid: Some(-2),
        skill_id: Some(skill_id),
        ..Default::default()
    };
    let player_card = || CardInfo {
        uid: Some(12),
        skill_id: Some(202),
        ..Default::default()
    };
    let normal = {
        let mut ex_point = crate::engine::manager::ex_point::ExPointManager::default();
        ex_point.seed(&fight);
        let mut eureka = crate::engine::manager::eureka::EurekaManager::default();
        eureka.seed(&fight);
        start_decks_from_fight(
            crate::test_support::game_data(),
            &fight,
            &ex_point,
            &eureka,
            1,
            0,
            None,
        )
    };

    for captured in [
        (vec![ai_card(302), ai_card(303)], vec![player_card()]),
        (
            vec![ai_card(302), ai_card(303)],
            vec![
                player_card(),
                CardInfo {
                    uid: Some(12),
                    skill_id: Some(999),
                    ..Default::default()
                },
                player_card(),
            ],
        ),
    ] {
        let mut ex_point = crate::engine::manager::ex_point::ExPointManager::default();
        ex_point.seed(&fight);
        let mut eureka = crate::engine::manager::eureka::EurekaManager::default();
        eureka.seed(&fight);
        assert_eq!(
            start_decks_from_fight(
                crate::test_support::game_data(),
                &fight,
                &ex_point,
                &eureka,
                1,
                0,
                Some(captured),
            ),
            normal
        );
    }
}

#[test]
fn draw_bag_keeps_sixteen_balanced_rank_one_cards_per_hero() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1001, 1, &[101], &[201])],
            ..Default::default()
        }),
        ..Default::default()
    };

    let bag = draw_bag(crate::test_support::game_data(), &fight);

    assert_eq!(bag.len(), 16);
    assert_eq!(
        bag.iter().filter(|card| card.skill_id == Some(101)).count(),
        8
    );
    assert_eq!(
        bag.iter().filter(|card| card.skill_id == Some(201)).count(),
        8
    );
}

#[test]
fn configured_opening_deals_do_not_change_version_six_replays() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let fight = Fight {
        episode_id: Some(10002),
        version: Some(6),
        attacker: Some(FightTeam {
            entitys: vec![
                entity(-1, 100102, 1, &[30250111], &[30250121]),
                entity(-2, 100101, 2, &[30230111], &[30230121]),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };

    assert!(
        configured_opening_deal(crate::test_support::game_data(), &fight)
            .unwrap()
            .is_none()
    );
    assert!(
        configured_refill_draws(crate::test_support::game_data(), &fight)
            .unwrap()
            .is_empty()
    );
    assert!(opening_deal(catalog, &fight).unwrap().is_none());
    assert!(refill_draws(catalog, &fight).unwrap().is_empty());
}

#[test]
fn configured_device_cards_extend_the_draw_bag_without_inflating_the_normal_deck() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 3149, 1, &[31490111], &[31490131])],
            ..Default::default()
        }),
        ..Default::default()
    };

    let bag = draw_bag(crate::test_support::game_data(), &fight);
    let configured = configured_draw_bag(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        &fight,
    );

    assert_eq!(configured, bag);

    assert_eq!(deck_size(&fight), 16);
    assert_eq!(bag.len(), 26);
    assert_eq!(
        bag.iter()
            .filter(|card| card.skill_id == Some(31446011))
            .count(),
        2
    );
    assert_eq!(
        bag.iter()
            .filter(|card| card.skill_id == Some(31490201))
            .count(),
        1
    );
}

fn entity(
    uid: i64,
    model_id: i32,
    position: i32,
    skill_group1: &[i32],
    skill_group2: &[i32],
) -> FightEntityInfo {
    FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        position: Some(position),
        current_hp: Some(100),
        skill_group1: skill_group1.to_vec(),
        skill_group2: skill_group2.to_vec(),
        ..Default::default()
    }
}
