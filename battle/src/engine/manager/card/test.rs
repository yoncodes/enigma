use super::*;
use rand::{SeedableRng, rngs::StdRng};
use sonettobuf::{Fight, FightEntityInfo, FightTeam};

#[test]
fn enemy_ai_selects_ultimates_from_current_resource_state() {
    let fight = |ex_point, ex_skill| Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(100),
                current_hp: Some(100),
                ex_point: Some(ex_point),
                ex_skill,
                skill_group1: vec![100],
                skill_group2: vec![200],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };

    let resources = |fight: &Fight| {
        let mut ex_point = crate::engine::manager::ex_point::ExPointManager::default();
        ex_point.seed(fight);
        let mut eureka = crate::engine::manager::eureka::EurekaManager::default();
        eureka.seed(fight);
        (ex_point, eureka)
    };

    let ready = fight(5, Some(900));
    let (ready_ex_point, ready_eureka) = resources(&ready);
    let mut ready_rng = StdRng::seed_from_u64(1);
    assert_eq!(
        ai::generate_ai_deck(&ready, &ready_ex_point, &ready_eureka, &mut ready_rng,)[0].skill_id,
        Some(900)
    );

    let fallback = fight(5, None);
    let (fallback_ex_point, fallback_eureka) = resources(&fallback);
    let mut fallback_rng = StdRng::seed_from_u64(1);
    assert_eq!(
        ai::generate_ai_deck(
            &fallback,
            &fallback_ex_point,
            &fallback_eureka,
            &mut fallback_rng,
        )[0]
        .skill_id,
        Some(100)
    );

    let stale_fight = fight(5, Some(900));
    let (spent_ex_point, spent_eureka) = resources(&fight(0, Some(900)));
    let mut spent_rng = StdRng::seed_from_u64(1);
    assert_eq!(
        ai::generate_ai_deck(&stale_fight, &spent_ex_point, &spent_eureka, &mut spent_rng,)[0]
            .skill_id,
        Some(100)
    );
}

#[test]
fn enemy_ai_selects_boss_ultimate_from_full_named_power() {
    let fight = |power| Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ex_point: Some(0),
                ex_skill: Some(900),
                skill_group1: vec![100],
                power_infos: vec![sonettobuf::PowerInfo {
                    power_id: Some(
                        crate::engine::manager::eureka::PowerType::ZongMaoBossEnergy.id(),
                    ),
                    num: Some(power),
                    max: Some(3),
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };

    for (power, expected) in [(2, 100), (3, 900)] {
        let fight = fight(power);
        let mut ex_point = crate::engine::manager::ex_point::ExPointManager::default();
        ex_point.seed(&fight);
        let mut eureka = crate::engine::manager::eureka::EurekaManager::default();
        eureka.seed(&fight);
        let mut rng = StdRng::seed_from_u64(1);
        assert_eq!(
            ai::generate_ai_deck(&fight, &ex_point, &eureka, &mut rng)[0].skill_id,
            Some(expected)
        );
    }
}

#[test]
fn moves_cards_before_playing() {
    let mut cards = CardManager::new(vec![card(10, 100), card(11, 200), card(12, 300)]);

    cards.move_card(0, 2);
    let plan = cards.play_card(0, Some(-1), None, None);

    assert_eq!(
        plan,
        Some(PlayedCard {
            target_uid: Some(-1),
            card_index: 1,
            skill_id: 200,
            rank_change_pending: false,
            rewritten: false,
            card: card(11, 200),
            caster_uid: 11,
            recorded_skill: None,
        })
    );
    assert_eq!(
        cards.hand().iter().map(|card| card.uid).collect::<Vec<_>>(),
        vec![Some(12), Some(10)]
    );
    assert_eq!(cards.played()[0].skill_id, 200);
    assert_eq!(cards.played_skill_counts(11), vec![(200, 1)]);
}

#[test]
fn fight_step_card_index_is_play_order_not_hand_slot() {
    let mut cards = CardManager::new(vec![card(10, 100), card(11, 200), card(12, 300)]);

    let first = cards.play_card(2, None, None, None).unwrap();
    let second = cards.play_card(0, None, None, None).unwrap();

    assert_eq!((first.card_index, second.card_index), (1, 2));
}

#[test]
fn queued_rank_up_uses_the_cards_owner_skill_group() {
    let mut cards = CardManager::new(vec![card(10, 100)]);
    cards.rank_up.insert((10, 100), 101);
    cards.rank_up.insert((10, 101), 102);
    let played = cards.play_card(0, None, None, None).unwrap();

    assert_eq!(cards.rank_up_played(played.card_index, 2, true), Some(102));
    assert_eq!(cards.played()[0].skill_id, 102);
    assert_eq!(cards.played()[0].card.skill_id, Some(100));

    let changes = cards.resolve_played_ranks();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].card.skill_id, Some(102));
    assert!(changes[0].rewritten);
}

#[test]
fn queued_rank_up_changes_only_the_requested_following_cards() {
    let mut cards = CardManager::new(vec![card(10, 100), card(11, 200), card(12, 300)]);
    cards.rank_up.insert((11, 200), 201);
    cards.rank_up.insert((12, 300), 301);
    cards.play_card(0, None, None, None).unwrap();
    cards.play_card(0, None, None, None).unwrap();
    cards.play_card(0, None, None, None).unwrap();

    cards.rank_up_played_after(1, 1, 1);

    assert_eq!(
        cards
            .played()
            .iter()
            .map(|played| played.skill_id)
            .collect::<Vec<_>>(),
        vec![100, 201, 300]
    );
}

#[test]
fn selecting_an_alternate_skill_does_not_create_a_rank_change() {
    let mut cards = CardManager::new(vec![card(10, 100)]);

    let played = cards.play_card(0, None, Some(900), None).unwrap();

    assert_eq!(played.skill_id, 900);
    assert_eq!(played.card.skill_id, Some(100));
    assert!(cards.resolve_played_ranks().is_empty());
}

#[test]
fn chosen_skill_overrides_choice_card_skill() {
    let mut cards = CardManager::new(vec![card(10, 100)]);

    let plan = cards.play_card(0, None, Some(999), None);

    assert_eq!(plan.unwrap().skill_id, 999);
}

#[test]
fn card_energy_only_changes_basic_incantations() {
    let mut basic = card(10, 100);
    basic.card_effect = Some(1);
    let mut ultimate = card(10, 200);
    ultimate.card_effect = Some(0);
    let mut temp = basic.clone();
    temp.temp_card = Some(true);
    let mut cards = CardManager::new(vec![basic, ultimate, temp]);

    cards.add_basic_card_energy(-1, 1);

    assert_eq!(
        cards
            .hand()
            .iter()
            .map(|card| card.energy)
            .collect::<Vec<_>>(),
        vec![Some(-1), None, None]
    );
}

#[test]
fn clearing_card_energy_updates_the_owned_hand() {
    let mut first = card(10, 100);
    first.energy = Some(3);
    let mut second = card(11, 200);
    second.energy = Some(1);
    let mut cards = CardManager::new(vec![first, second]);

    cards.clear_energy();

    assert!(cards.hand().iter().all(|card| card.energy == Some(0)));
}

#[test]
fn universal_card_cannot_merge_a_lorenz_rewritten_card() {
    let universal = card(0, UniversalCardSkill::RankOne.id());
    let mut rewritten = card(10, 100);
    rewritten.enchants.push(sonettobuf::CardEnchant {
        enchant_id: Some(EnchantedType::Lorenz.id()),
        ..Default::default()
    });
    let mut cards = CardManager::new(vec![universal, rewritten]);
    cards.rank_up.insert((10, 100), 101);

    assert_eq!(cards.use_universal(0, 1), None);
    assert_eq!(cards.hand().len(), 2);
}

#[test]
fn captured_choice_prefers_the_client_slot_between_duplicate_sources() {
    let source = card(10, 100);
    let mut cards = CardManager::new(vec![source.clone(), card(11, 200), source.clone()]);

    let plan = cards.play_card(
        0,
        Some(-1),
        None,
        Some(CardPlayChoice {
            source,
            played: card(12, 999),
        }),
    );

    assert_eq!(plan.unwrap().skill_id, 999);
    assert_eq!(cards.hand(), &[card(11, 200), card(10, 100)]);
}

#[test]
fn captured_choice_does_not_replace_manager_owned_card_identity() {
    let mut cards = CardManager::new(vec![card(10, 100)]);
    let captured = card(10, 200);

    let played = cards
        .play_card(
            0,
            Some(-1),
            None,
            Some(CardPlayChoice {
                source: captured.clone(),
                played: captured.clone(),
            }),
        )
        .unwrap();

    assert_eq!(played.card, card(10, 100));
    assert_eq!(played.caster_uid, 10);
    assert_eq!(played.skill_id, 200);
    assert!(cards.hand().is_empty());
}

#[test]
fn rewritten_choice_keeps_consumed_card_and_resolved_caster_distinct() {
    let source = card(10, 100);
    let mut cards = CardManager::new(vec![source.clone()]);

    let played = cards
        .play_card(
            0,
            Some(-1),
            None,
            Some(CardPlayChoice {
                source: card(20, 200),
                played: card(20, 200),
            }),
        )
        .unwrap();

    assert_eq!(played.card, source);
    assert_eq!(played.caster_uid, 20);
    assert_eq!(played.skill_id, 200);
    assert!(cards.hand().is_empty());
    assert_eq!(cards.played_skill_counts(10), Vec::new());
    assert_eq!(cards.played_skill_counts(20), vec![(200, 1)]);
}

#[test]
fn wire_temp_card_uid_does_not_erase_its_manager_owned_caster() {
    let source = precast_card(10, 900);
    let wire = temp_card(900);
    let mut cards = CardManager::new(vec![source.clone()]);

    let played = cards
        .play_card(
            0,
            Some(-1),
            None,
            Some(CardPlayChoice {
                source: wire.clone(),
                played: wire,
            }),
        )
        .unwrap();

    assert_eq!(played.card, source);
    assert_eq!(played.skill_id, 900);
}

#[test]
fn recorded_skill_survives_action_queue_card_removal() {
    let recorded = crate::engine::skill::action::SkillRequest {
        source_uid: 11,
        skill_id: 200,
    };
    let mut cards = CardManager::new(vec![card(10, 900), card(11, 200)]);

    let ultimate = cards
        .play_card_with_record(0, None, None, None, Some(recorded))
        .unwrap();
    cards.play_card(0, None, None, None).unwrap();

    assert_eq!(ultimate.recorded_skill, Some(recorded));
    assert!(cards.hand().is_empty());

    let mut cards = CardManager::new(vec![card(10, 900)]);
    let spoofed = cards
        .play_card_with_record(
            0,
            None,
            None,
            None,
            Some(crate::engine::skill::action::SkillRequest {
                source_uid: 11,
                skill_id: 200,
            }),
        )
        .unwrap();
    assert_eq!(spoofed.recorded_skill, None);
}

#[test]
fn team_card_is_playable_without_counting_as_a_hand_card() {
    let team_card = card(10, 900);
    let mut cards = CardManager::new(vec![card(10, 100)]);
    cards.set_team_cards(vec![team_card.clone()]);

    let played = cards.play_card(
        1,
        Some(-1),
        None,
        Some(CardPlayChoice {
            source: team_card.clone(),
            played: team_card,
        }),
    );

    assert_eq!(played.unwrap().skill_id, 900);
    assert_eq!(cards.hand(), &[card(10, 100)]);
    assert!(cards.team_cards().is_empty());
}

#[test]
fn draw_add_temp_compose_and_dissolve_update_hand_state() {
    let mut cards =
        CardManager::with_draw_pile(vec![card(10, 100)], vec![card(11, 200), card(12, 300)]);

    assert_eq!(cards.draw(1)[0].uid, Some(11));
    assert_eq!(cards.hand().len(), 2);
    assert_eq!(cards.draw_pile().len(), 1);

    let temp = cards.add_temp_card(999);
    assert_eq!(temp.temp_card, Some(true));
    assert_eq!(cards.generated().len(), 1);

    let composed = cards.compose(&[0, 1], card(13, 400)).unwrap();
    assert_eq!(composed.skill_id, Some(400));
    assert_eq!(cards.hand().len(), 2);

    let removed = cards.dissolve(0, Some(card(14, 500))).unwrap();
    assert_eq!(removed.skill_id, Some(999));
    assert_eq!(cards.hand().last().unwrap().skill_id, Some(500));
}

#[test]
fn change_to_temp_replaces_card_in_place() {
    let mut cards = CardManager::new(vec![card(10, 100)]);

    let changed = cards.change_to_temp_card(0, 777).unwrap();

    assert_eq!(changed.skill_id, Some(777));
    assert_eq!(cards.hand()[0].uid, Some(0));
    assert_eq!(cards.hand()[0].temp_card, Some(true));
}

#[test]
fn refill_composes_adjacent_cards_and_reports_the_moxie_owner() {
    let mut cards = CardManager::new(vec![card(10, 100)]);
    cards.rank_up.insert((10, 100), 101);

    let mut draws = vec![card(10, 100), card(11, 200)].into_iter();
    let refill = cards.refill_to(3, || draws.next());

    assert_eq!(refill.drawn.len(), 2);
    assert_eq!(refill.composed_owners, vec![10]);
    assert_eq!(cards.hand()[0].skill_id, Some(101));
    assert_eq!(cards.hand()[0].temp_card, Some(false));
    assert_eq!(cards.hand()[0].energy, Some(0));
    assert_eq!(cards.hand()[1], card(11, 200));
}

#[test]
fn removing_an_owner_composes_the_new_ai_queue_neighbors() {
    let mut manager = CardManager::default();
    manager.rank_up.insert((10, 100), 101);
    manager.set_ai_queue(vec![card(10, 100), card(11, 200), card(10, 100)]);

    let composed = manager.remove_ai_owner_cards(11);

    assert_eq!(composed, Some(vec![10]));
    assert_eq!(manager.remove_ai_owner_cards(11), None);
    assert_eq!(manager.ai_queue()[0].skill_id, Some(101));
    assert_eq!(manager.ai_queue()[0].temp_card, Some(false));
    assert_eq!(manager.ai_queue()[0].energy, Some(0));
}

#[test]
fn refill_keeps_drawing_after_compositions_until_the_hand_is_full() {
    let mut cards = CardManager::new(vec![card(1, 10), card(2, 20), card(3, 30), card(4, 40)]);
    cards.rank_up.insert((2, 20), 21);
    cards.rank_up.insert((3, 31), 32);
    let mut draws = vec![
        card(2, 20),
        card(2, 20),
        card(1, 11),
        card(3, 31),
        card(3, 31),
        card(4, 41),
    ]
    .into_iter();

    let refill = cards.refill_to(8, || draws.next());

    assert_eq!(refill.drawn.len(), 6);
    assert_eq!(refill.composed_owners, vec![2, 3]);
    assert_eq!(cards.hand().len(), 8);
    assert!(cards.hand().iter().any(|card| card.skill_id == Some(21)));
    assert!(cards.hand().iter().any(|card| card.skill_id == Some(32)));
}

fn card(uid: i64, skill_id: i32) -> CardInfo {
    CardInfo {
        uid: Some(uid),
        skill_id: Some(skill_id),
        ..Default::default()
    }
}
