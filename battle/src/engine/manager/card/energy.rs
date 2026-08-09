use sonettobuf::CardInfo;

use crate::engine::{
    manager::{buff::BuffManager, hp::HpManager},
    runtime::determinism::RoundDeterminism,
    skill::effect::SkillEffectCatalog,
};

const MAX_CARD_ENERGY: i32 = 5;
const BASE_WEIGHT: i32 = 100;

pub fn allocate(
    buffs: &BuffManager,
    hp: &HpManager,
    catalog: &SkillEffectCatalog,
    cards: &[CardInfo],
    available: i32,
    use_priority_rule: bool,
    determinism: &mut RoundDeterminism,
) -> Option<Vec<CardInfo>> {
    let battle_catalog = buffs
        .try_catalog()
        .or_else(crate::catalog::BattleCatalog::try_global);
    if available <= 0 {
        return None;
    }

    let eligible = eligible_cards(cards, catalog, battle_catalog, use_priority_rule);
    if eligible.is_empty() {
        return None;
    }

    let mut output = cards.to_vec();
    let before = card_energy(&output);
    let capacity = eligible
        .iter()
        .map(|(index, _)| {
            MAX_CARD_ENERGY
                - output[*index]
                    .energy
                    .unwrap_or_default()
                    .clamp(0, MAX_CARD_ENERGY)
        })
        .sum::<i32>();
    let mut remaining = available.min(capacity);
    if use_priority_rule {
        let features = buffs.active_features(hp);
        while remaining > 0 {
            let candidates = eligible
                .iter()
                .filter(|(index, _)| output[*index].energy != Some(MAX_CARD_ENERGY))
                .map(|(index, card)| {
                    let bonus = features
                        .iter()
                        .map(|feature| {
                            crate::engine::skill::buff_act::emitter_card_allocate_change::configured_weight_bonus(
                                feature,
                                catalog,
                                battle_catalog,
                                card.skill_id.unwrap_or_default(),
                            )
                        })
                        .sum::<i32>();
                    (*index, BASE_WEIGHT.saturating_add(bonus).max(1))
                })
                .collect::<Vec<_>>();
            let Some(index) = weighted_index(&candidates, determinism) else {
                break;
            };
            remaining -= add_energy(&mut output, index, 1);
        }
    } else {
        let mut index = 0;
        while remaining > 0 && !eligible.is_empty() {
            remaining -= add_energy(&mut output, eligible[index].0, 1);
            index = (index + 1) % eligible.len();
        }
    }

    (card_energy(&output) > before).then_some(output)
}

fn eligible_cards<'a>(
    cards: &'a [CardInfo],
    catalog: &SkillEffectCatalog,
    battle_catalog: Option<crate::catalog::BattleCatalog>,
    weighted: bool,
) -> Vec<(usize, &'a CardInfo)> {
    cards
        .iter()
        .enumerate()
        .skip(if weighted { 0 } else { 2 })
        .filter(|(_, card)| !card.temp_card.unwrap_or(false))
        .filter(|(_, card)| {
            card.skill_id.is_some_and(|skill_id| {
                !catalog.is_big_skill(skill_id)
                    && (1..=3).contains(
                        &battle_catalog
                            .map(|catalog| catalog.card_skill_rank(card))
                            .unwrap_or_else(|| card.card_effect.unwrap_or_default()),
                    )
            })
        })
        .collect()
}

fn weighted_index(
    candidates: &[(usize, i32)],
    determinism: &mut RoundDeterminism,
) -> Option<usize> {
    let total: usize = candidates.iter().map(|(_, weight)| *weight as usize).sum();
    let mut roll = determinism.lua_random_index(total)?;
    for &(index, weight) in candidates {
        if roll < weight as usize {
            return Some(index);
        }
        roll -= weight as usize;
    }
    None
}

fn add_energy(cards: &mut [CardInfo], index: usize, amount: i32) -> i32 {
    let Some(card) = cards.get_mut(index) else {
        return 0;
    };
    let current = card.energy.unwrap_or_default();
    let added = amount.min(MAX_CARD_ENERGY - current).max(0);
    card.energy = Some(current + added);
    added
}

fn card_energy(cards: &[CardInfo]) -> i32 {
    cards
        .iter()
        .map(|card| card.energy.unwrap_or_default())
        .sum()
}

#[cfg(test)]
mod tests {
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
            99,
            false,
            &mut RoundDeterminism::default(),
        )
        .unwrap();

        assert_eq!(
            allocated.iter().map(|card| card.energy).collect::<Vec<_>>(),
            vec![Some(4), Some(4), Some(5)]
        );
    }
}
