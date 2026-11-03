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
