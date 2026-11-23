use super::*;

#[test]
fn allocation_preserves_old_energy_and_stops_at_card_capacity() {
    let cards = (1..=3)
        .map(|uid| CardInfo {
            uid: Some(uid),
            skill_id: Some(uid as i32),
            card_effect: Some(1),
            energy: Some(4),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    let allocated = allocate(
        &BuffManager::default(),
        &HpManager::default(),
        &SkillEffectCatalog::default(),
        &cards,
        3,
        BASE_LIMIT,
        &mut RoundDeterminism::default(),
    )
    .unwrap();

    assert_eq!(
        allocated.iter().map(|card| card.energy).collect::<Vec<_>>(),
        vec![Some(4), Some(4), Some(5)]
    );
}

#[test]
fn weighted_allocation_replays_an_observed_hand() {
    let card = |uid: i64, energy| CardInfo {
        uid: Some(uid),
        skill_id: Some(uid as i32),
        card_effect: Some(1),
        energy: Some(energy),
        ..Default::default()
    };
    let cards = vec![card(1, 0), card(2, 0), card(3, 0), card(4, 0)];
    let observed = vec![card(1, 0), card(2, 4), card(3, 1), card(4, 1)];
    let wrong_total = vec![card(1, 0), card(2, 5), card(3, 1), card(4, 1)];
    let allocate_with = |hands: Vec<Vec<CardInfo>>| {
        let mut determinism = RoundDeterminism::default();
        determinism.replace_card_energy_allocations(hands);
        let allocated = allocate(
            &BuffManager::default(),
            &HpManager::default(),
            &SkillEffectCatalog::default(),
            &cards,
            6,
            BASE_LIMIT,
            &mut determinism,
        )
        .unwrap();
        (
            allocated.iter().map(|card| card.energy).collect::<Vec<_>>(),
            determinism.take_card_energy_allocation(|_| true).is_some(),
        )
    };

    assert_eq!(
        allocate_with(vec![wrong_total.clone(), observed.clone()]),
        (vec![Some(0), Some(4), Some(1), Some(1)], true)
    );
    let (rolled, unused) = allocate_with(vec![wrong_total]);
    assert_eq!(rolled.iter().flatten().sum::<i32>(), 6);
    assert!(unused);
}

#[test]
fn observed_hands_must_match_the_hand_and_the_weighted_path() {
    let card = |uid: i64, energy| CardInfo {
        uid: Some(uid),
        skill_id: Some(uid as i32),
        card_effect: Some(1),
        energy: Some(energy),
        ..Default::default()
    };
    let cards = vec![card(1, 0), card(2, 0), card(3, 0), card(4, 0)];
    let unused_after = |hand: Vec<CardInfo>, available| {
        let mut determinism = RoundDeterminism::default();
        determinism.replace_card_energy_allocations([hand]);
        allocate(
            &BuffManager::default(),
            &HpManager::default(),
            &SkillEffectCatalog::default(),
            &cards,
            available,
            BASE_LIMIT,
            &mut determinism,
        )
        .unwrap();
        determinism.take_card_energy_allocation(|_| true).is_some()
    };
    let other_card = vec![card(1, 0), card(2, 4), card(3, 1), card(5, 1)];
    let round_robin = vec![card(1, 1), card(2, 1), card(3, 1), card(4, 1)];

    assert!(unused_after(other_card, 6));
    assert!(unused_after(round_robin, 4));
}
