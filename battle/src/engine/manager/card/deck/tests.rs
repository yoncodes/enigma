use super::*;

#[test]
fn owns_hand_draw_and_card_mutations() {
    let mut deck =
        CardDeck::with_draw_pile(vec![card(10, 100)], vec![card(11, 200), card(12, 300)]);

    assert_eq!(deck.draw(1)[0].uid, Some(11));
    deck.move_card(0, 1);
    assert_eq!(
        deck.hand().iter().map(|card| card.uid).collect::<Vec<_>>(),
        vec![Some(11), Some(10)]
    );

    let temp = deck.add_temp_card(10, 999, None);
    assert_eq!(temp.temp_card, Some(true));
    assert_eq!(deck.generated().len(), 1);

    let composed = deck.compose(&[0, 1], card(13, 400)).unwrap();
    assert_eq!(composed.skill_id, Some(400));

    let removed = deck.dissolve(0, Some(card(14, 500))).unwrap();
    assert_eq!(removed.skill_id, Some(999));
    assert_eq!(deck.hand().last().unwrap().skill_id, Some(500));
}

#[test]
fn device_weights_survive_opening_deals_and_normal_recycle() {
    crate::test_support::init_config();
    let normal = card(10, 100);
    let power = card(10, 31446062);
    let special = card(10, 31450181);
    let mut deck = CardDeck::with_draw_pile(
        Vec::new(),
        vec![normal.clone(), power.clone(), special.clone()],
    )
    .with_catalog(crate::catalog::BattleCatalog::new(
        crate::test_support::game_data(),
    ));

    assert!(deck.deal_from_draw_pile(&[
        power.clone(),
        special.clone(),
        power.clone(),
        normal.clone(),
    ]));
    assert_eq!(deck.draw_pile(), &[power.clone(), special.clone()]);
    assert!(deck.recycle_draw_pile());
    assert_eq!(deck.draw_pile(), &[normal, power, special]);
}

#[test]
fn adjacent_equal_cards_compose_and_report_the_owner() {
    let mut deck = CardDeck::new(vec![card(10, 100), card(10, 100), card(11, 200)]);

    assert_eq!(
        deck.compose_adjacent(&HashMap::from([((10, 100), 101)])),
        vec![10]
    );
    assert_eq!(deck.hand().len(), 2);
    assert_eq!(deck.hand()[0].uid, Some(10));
    assert_eq!(deck.hand()[0].skill_id, Some(101));
    assert_eq!(deck.hand()[0].temp_card, Some(false));
    assert_eq!(deck.hand()[1], card(11, 200));
}

#[test]
fn adjacent_ultimate_cards_require_the_skill_three_lane() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let hand = vec![card(10, 30020131), card(10, 30020131)];
    let rank_up = HashMap::from([((10, 30020131), 30020132)]);
    let mut legacy = CardDeck::new(hand.clone());
    let mut explicit = CardDeck::new(hand).with_catalog(catalog);

    assert!(legacy.compose_adjacent(&rank_up).is_empty());
    assert!(explicit.compose_adjacent(&rank_up).is_empty());

    legacy.hand[0].card_type = Some(CardType::Skill3 as i32);
    explicit.hand[0].card_type = Some(CardType::Skill3 as i32);
    assert_eq!(legacy.compose_adjacent(&rank_up), vec![10]);
    assert_eq!(explicit.compose_adjacent(&rank_up), vec![10]);
    assert_eq!(explicit, legacy);
}

#[test]
fn temporary_lifetime_follows_a_card_when_the_hand_moves() {
    let mut deck = CardDeck::new(vec![card(10, 100), card(11, 200)]);
    let marked = deck.mark_temporary(&[0]).unwrap();

    assert!(deck.move_card(0, 1));
    deck.hand_mut()[1].energy = Some(5);
    let expired = deck.expire_temporary(&marked, &[]);

    assert_eq!(expired, [1]);
    assert_eq!(deck.hand(), &[card(11, 200)]);
}

#[test]
fn removing_an_owner_clears_every_deck_zone_without_composing() {
    let mut deck = CardDeck::with_draw_pile(
        vec![card(10, 100), card(11, 200), card(10, 100)],
        vec![card(10, 300), card(12, 400)],
    );
    assert!(deck.consume_draw_card(&card(10, 300)));
    deck.add_temp_card(10, 500, None);

    deck.remove_owner_cards(10);

    assert_eq!(deck.hand, vec![card(11, 200)]);
    assert_eq!(deck.hand_ids.len(), 1);
    assert_eq!(deck.draw_pile, vec![card(12, 400)]);
    assert!(deck.discard_pile.is_empty());
    assert!(deck.generated.is_empty());
}

#[test]
fn adjacent_compose_respects_non_combine_enchants_and_merges_card_state() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let mut blocked = card(10, 100);
    blocked.enchants.push(CardEnchant {
        enchant_id: Some(10_010),
        duration: Some(1),
        ..Default::default()
    });
    let blocked_hand = vec![blocked, card(10, 100)];
    let mut deck = CardDeck::new(blocked_hand.clone());
    let mut explicit = CardDeck::new(blocked_hand).with_catalog(catalog);
    let rank_up = HashMap::from([((10, 100), 101)]);

    assert!(deck.compose_adjacent(&rank_up).is_empty());
    assert_eq!(explicit.compose_adjacent(&rank_up), Vec::<i64>::new());
    assert_eq!(explicit, deck);
    assert_eq!(deck.hand().len(), 2);

    let mut left = card(10, 100);
    left.temp_card = Some(true);
    left.energy = Some(2);
    let mut right = card(10, 100);
    right.temp_card = Some(false);
    right.energy = Some(3);
    right.enchants.push(CardEnchant {
        enchant_id: Some(10_006),
        duration: Some(2),
        ..Default::default()
    });
    let merge_hand = vec![left, right];
    let mut deck = CardDeck::new(merge_hand.clone());
    let mut explicit = CardDeck::new(merge_hand).with_catalog(catalog);

    assert_eq!(deck.compose_adjacent(&rank_up), vec![10]);
    assert_eq!(explicit.compose_adjacent(&rank_up), vec![10]);
    assert_eq!(explicit, deck);
    assert_eq!(deck.hand()[0].temp_card, Some(false));
    assert_eq!(deck.hand()[0].energy, Some(5));
    assert_eq!(deck.hand()[0].enchants[0].enchant_id, Some(10_006));
}

#[test]
fn attached_catalog_preserves_enchant_rejection() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let mut enchanted = card(10, 100);
    enchanted.enchants.push(CardEnchant {
        enchant_id: Some(EnchantedType::Chaos.id()),
        duration: Some(1),
        ..Default::default()
    });
    let mut legacy = CardDeck::new(vec![enchanted.clone()]);
    let mut explicit = CardDeck::new(vec![enchanted]).with_catalog(catalog);

    assert_eq!(
        explicit.enchant(&[0], EnchantedType::Precision, 2),
        legacy.enchant(&[0], EnchantedType::Precision, 2)
    );
    assert_eq!(explicit.hand(), legacy.hand());
    assert_eq!(explicit.hand()[0].enchants.len(), 1);
}

fn card(uid: i64, skill_id: i32) -> CardInfo {
    CardInfo {
        uid: Some(uid),
        skill_id: Some(skill_id),
        ..Default::default()
    }
}

#[test]
fn replaced_hand_keeps_identity_for_unchanged_slots() {
    let mut deck = CardDeck::new(vec![card(10, 1), card(10, 2)]);
    let before = deck.hand_ids.clone();

    deck.replace_hand(vec![card(10, 1), card(10, 3), card(10, 4)]);

    assert_eq!(deck.hand_ids[0], before[0]);
    assert!(!before.contains(&deck.hand_ids[1]));
    assert!(!before.contains(&deck.hand_ids[2]));
    assert_ne!(deck.hand_ids[1], deck.hand_ids[2]);
}
