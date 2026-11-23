use std::collections::{HashMap, HashSet};

use sonettobuf::{CardEnchant, CardInfo, card_info::CardType};

use super::{EnchantedType, permissions, precast_card, temp_card};

#[derive(Debug, Clone)]
pub struct CardDeck {
    catalog_data: Option<crate::catalog::BattleCatalog>,
    hand: Vec<CardInfo>,
    // In-place card mutations preserve identity; structural replacements allocate a new one.
    hand_ids: Vec<CardInstanceId>,
    next_hand_id: u64,
    draw_pile: Vec<CardInfo>,
    discard_pile: Vec<CardInfo>,
    generated: Vec<CardInfo>,
}

impl PartialEq for CardDeck {
    fn eq(&self, other: &Self) -> bool {
        self.hand == other.hand
            && self.hand_ids == other.hand_ids
            && self.next_hand_id == other.next_hand_id
            && self.draw_pile == other.draw_pile
            && self.discard_pile == other.discard_pile
            && self.generated == other.generated
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct CardInstanceId(u64);

impl Default for CardDeck {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl CardDeck {
    pub fn new(hand: Vec<CardInfo>) -> Self {
        let mut deck = Self {
            catalog_data: None,
            hand: Vec::new(),
            hand_ids: Vec::new(),
            next_hand_id: 1,
            draw_pile: Vec::new(),
            discard_pile: Vec::new(),
            generated: Vec::new(),
        };
        for card in hand {
            deck.push_hand(card);
        }
        deck
    }

    pub fn with_draw_pile(hand: Vec<CardInfo>, draw_pile: Vec<CardInfo>) -> Self {
        let mut deck = Self {
            catalog_data: None,
            hand: Vec::new(),
            hand_ids: Vec::new(),
            next_hand_id: 1,
            draw_pile,
            discard_pile: Vec::new(),
            generated: Vec::new(),
        };
        for card in hand {
            deck.push_hand(card);
        }
        deck
    }

    pub(crate) fn with_catalog(mut self, catalog: crate::catalog::BattleCatalog) -> Self {
        self.catalog_data = Some(catalog);
        self
    }

    pub(super) fn catalog(&self) -> Option<crate::catalog::BattleCatalog> {
        self.catalog_data
            .or_else(crate::catalog::BattleCatalog::try_global)
    }

    pub(super) fn attached_catalog(&self) -> Option<crate::catalog::BattleCatalog> {
        self.catalog_data
    }

    pub fn hand(&self) -> &[CardInfo] {
        &self.hand
    }

    pub(super) fn hand_mut(&mut self) -> &mut [CardInfo] {
        &mut self.hand
    }

    pub(super) fn hand_id(&self, index: usize) -> Option<CardInstanceId> {
        self.hand_ids.get(index).copied()
    }

    pub fn into_hand(self) -> Vec<CardInfo> {
        self.hand
    }

    pub fn generated(&self) -> &[CardInfo] {
        &self.generated
    }

    pub fn draw_pile(&self) -> &[CardInfo] {
        &self.draw_pile
    }

    pub(super) fn draw_pile_mut(&mut self) -> &mut [CardInfo] {
        &mut self.draw_pile
    }

    pub fn consume_draw_card(&mut self, card: &CardInfo) -> bool {
        let Some(index) = self
            .draw_pile
            .iter()
            .position(|candidate| same_card(candidate, card))
        else {
            return false;
        };
        self.discard_pile.push(self.draw_pile.remove(index));
        true
    }

    pub(super) fn deal_from_draw_pile(&mut self, cards: &[CardInfo]) -> bool {
        let mut remaining = self.draw_pile.clone();
        for card in cards {
            let Some(index) = remaining
                .iter()
                .position(|candidate| same_card(candidate, card))
            else {
                return false;
            };
            if !self.is_persistent_draw_card(card) {
                remaining.remove(index);
            }
        }
        for card in cards {
            if !self.is_persistent_draw_card(card) {
                self.consume_draw_card(card);
            }
            self.push_hand(card.clone());
        }
        true
    }

    pub(super) fn recycle_draw_pile(&mut self) -> bool {
        if !self.can_recycle_draw_pile() {
            return false;
        }
        let persistent = std::mem::take(&mut self.draw_pile);
        self.draw_pile = std::mem::take(&mut self.discard_pile);
        self.draw_pile.extend(persistent);
        true
    }

    pub(super) fn can_recycle_draw_pile(&self) -> bool {
        self.draw_pile
            .iter()
            .all(|card| self.is_persistent_draw_card(card))
            && !self.discard_pile.is_empty()
    }

    fn is_persistent_draw_card(&self, card: &CardInfo) -> bool {
        self.catalog().is_some_and(|catalog| {
            card.skill_id.is_some_and(|skill_id| {
                catalog.skill_effect_tag(skill_id)
                    == crate::engine::skill::effect::catalog::SkillEffectTag::Device as i32
            })
        })
    }

    pub fn move_card(&mut self, from_index: usize, to_index: usize) -> bool {
        if from_index >= self.hand.len() || to_index >= self.hand.len() || from_index == to_index {
            return false;
        }

        let card = self.hand.remove(from_index);
        let card_id = self.hand_ids.remove(from_index);
        self.hand.insert(to_index, card);
        self.hand_ids.insert(to_index, card_id);
        true
    }

    pub fn replace_owner_skills(
        &mut self,
        owner_uid: i64,
        base_group1: &[i32],
        base_group2: &[i32],
        replacement_group1: &[i32],
        replacement_group2: &[i32],
    ) {
        for card in self
            .hand
            .iter_mut()
            .chain(&mut self.draw_pile)
            .chain(&mut self.discard_pile)
            .chain(&mut self.generated)
            .filter(|card| card.uid == Some(owner_uid))
        {
            let Some(skill_id) = card.skill_id else {
                continue;
            };
            let replacement = base_group1
                .iter()
                .position(|id| *id == skill_id)
                .and_then(|index| replacement_group1.get(index))
                .or_else(|| {
                    base_group2
                        .iter()
                        .position(|id| *id == skill_id)
                        .and_then(|index| replacement_group2.get(index))
                });
            if let Some(replacement) = replacement {
                card.skill_id = Some(*replacement);
            }
        }
    }

    pub(super) fn remove_owner_cards(&mut self, owner_uid: i64) {
        for index in (0..self.hand.len()).rev() {
            if self.hand[index].uid == Some(owner_uid) {
                self.remove_card(index);
            }
        }
        self.draw_pile.retain(|card| card.uid != Some(owner_uid));
        self.discard_pile.retain(|card| card.uid != Some(owner_uid));
        self.generated.retain(|card| card.uid != Some(owner_uid));
    }

    pub fn draw(&mut self, count: usize) -> Vec<CardInfo> {
        let take = count.min(self.draw_pile.len());
        let drawn: Vec<_> = self.draw_pile.drain(..take).collect();
        self.discard_pile.extend(drawn.iter().cloned());
        for card in drawn.iter().cloned() {
            self.push_hand(card);
        }
        drawn
    }

    pub fn add_to_hand(&mut self, card: CardInfo) -> CardInfo {
        self.push_hand(card.clone());
        card
    }

    pub fn compose_adjacent(&mut self, rank_up: &HashMap<(i64, i32), i32>) -> Vec<i64> {
        let mut owners = Vec::new();
        let mut index = 0;
        let catalog = self.catalog();
        while index + 1 < self.hand.len() {
            let left = &self.hand[index];
            let right = &self.hand[index + 1];
            let owner_uid = left.uid.unwrap_or_default();
            let Some(next_skill_id) = composable_next(catalog, left, right, rank_up) else {
                index += 1;
                continue;
            };

            let right = self.hand[index + 1].clone();
            self.hand[index].skill_id = Some(next_skill_id);
            self.hand[index].temp_card = Some(false);
            self.hand[index].energy = Some(
                self.hand[index]
                    .energy
                    .unwrap_or_default()
                    .saturating_add(right.energy.unwrap_or_default()),
            );
            merge_enchants(catalog, &mut self.hand[index].enchants, &right.enchants);
            self.remove_card(index + 1);
            owners.push(owner_uid);
            index = index.saturating_sub(1);
        }
        owners
    }

    pub fn add_temp_card(
        &mut self,
        owner_uid: i64,
        skill_id: i32,
        hero_id: Option<i32>,
    ) -> CardInfo {
        let card = hero_id.map_or_else(
            || {
                if owner_uid == 0 {
                    temp_card(skill_id)
                } else {
                    precast_card(owner_uid, skill_id)
                }
            },
            |hero_id| CardInfo {
                uid: Some(owner_uid),
                hero_id: Some(hero_id),
                card_type: Some(CardType::Skill3 as i32),
                ..temp_card(skill_id)
            },
        );
        self.push_hand(card.clone());
        self.generated.push(card.clone());
        card
    }

    pub fn change_to_temp_card(&mut self, index: usize, skill_id: i32) -> Option<CardInfo> {
        if index >= self.hand.len() {
            return None;
        }

        let card = temp_card(skill_id);
        self.hand[index] = card.clone();
        self.generated.push(card.clone());
        Some(card)
    }

    pub fn enchant(
        &mut self,
        indices: &[usize],
        enchant: EnchantedType,
        duration: i32,
    ) -> Option<Vec<CardInfo>> {
        if indices.is_empty()
            || indices.iter().any(|index| *index >= self.hand.len())
            || indices
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != indices.len()
        {
            return None;
        }
        let enchant = CardEnchant {
            enchant_id: Some(enchant.id()),
            duration: Some(duration),
            ..Default::default()
        };
        let catalog = self.catalog();
        Some(
            indices
                .iter()
                .map(|index| {
                    merge_enchants(
                        catalog,
                        &mut self.hand[*index].enchants,
                        std::slice::from_ref(&enchant),
                    );
                    self.hand[*index].clone()
                })
                .collect(),
        )
    }

    pub(super) fn mark_temporary(&mut self, indices: &[usize]) -> Option<Vec<CardInstanceId>> {
        if indices.is_empty()
            || indices.iter().any(|index| *index >= self.hand.len())
            || indices
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != indices.len()
        {
            return None;
        }
        for index in indices {
            self.hand[*index].temp_card = Some(true);
        }
        Some(indices.iter().map(|index| self.hand_ids[*index]).collect())
    }

    pub(super) fn expire_temporary(
        &mut self,
        card_ids: &[CardInstanceId],
        persistent_ids: &[CardInstanceId],
    ) -> Vec<usize> {
        let card_ids = card_ids.iter().copied().collect::<HashSet<_>>();
        let persistent_ids = persistent_ids.iter().copied().collect::<HashSet<_>>();
        let indices = self
            .hand_ids
            .iter()
            .enumerate()
            .filter_map(|(index, id)| card_ids.contains(id).then_some(index))
            .collect::<Vec<_>>();
        let persistent = indices
            .iter()
            .filter(|index| persistent_ids.contains(&self.hand_ids[**index]))
            .map(|index| CardInfo {
                temp_card: Some(false),
                ..self.hand[*index].clone()
            })
            .collect::<Vec<_>>();
        for index in indices.iter().rev() {
            self.remove_card(*index);
        }
        self.draw_pile.extend(persistent);
        indices
    }

    pub(super) fn persist_temporary(&mut self, indices: &[usize]) -> bool {
        if indices.is_empty()
            || indices.iter().any(|index| {
                self.hand
                    .get(*index)
                    .is_none_or(|card| !card.temp_card.unwrap_or_default())
            })
            || indices.iter().copied().collect::<HashSet<_>>().len() != indices.len()
        {
            return false;
        }
        for index in indices {
            self.hand[*index].temp_card = Some(false);
        }
        true
    }

    pub fn compose(&mut self, indices: &[usize], result: CardInfo) -> Option<CardInfo> {
        let mut indices = indices.to_vec();
        indices.sort_unstable();
        indices.dedup();
        if indices.len() < 2
            || indices.iter().any(|index| {
                self.hand
                    .get(*index)
                    .is_none_or(|card| !permissions::permits(card, permissions::NOT_COMPOSE))
            })
        {
            return None;
        }

        for index in indices.into_iter().rev() {
            self.remove_card(index);
        }
        self.push_hand(result.clone());
        Some(result)
    }

    pub fn combine_universal(
        &mut self,
        universal_index: usize,
        target_index: usize,
        result: CardInfo,
    ) -> Option<CardInfo> {
        if universal_index == target_index
            || universal_index >= self.hand.len()
            || target_index >= self.hand.len()
            || !permissions::permits(&self.hand[universal_index], permissions::NOT_COMPOSE)
            || !permissions::permits(&self.hand[target_index], permissions::NOT_COMPOSE)
        {
            return None;
        }
        let result_index = if universal_index < target_index {
            target_index - 1
        } else {
            target_index
        };
        self.remove_card(universal_index.max(target_index));
        self.remove_card(universal_index.min(target_index));
        self.insert_hand(result_index, result.clone())?;
        Some(result)
    }

    pub fn dissolve(&mut self, index: usize, replacement: Option<CardInfo>) -> Option<CardInfo> {
        let removed = self.remove_card(index)?;
        if let Some(card) = replacement {
            self.push_hand(card);
        }
        Some(removed)
    }

    pub fn take_for_play(&mut self, index: usize) -> Option<CardInfo> {
        self.remove_card(index)
    }

    pub fn take_matching(&mut self, card: &CardInfo) -> Option<CardInfo> {
        let index = self.matching_index(card)?;
        self.remove_card(index)
    }

    pub(super) fn matching_index(&self, card: &CardInfo) -> Option<usize> {
        self.hand
            .iter()
            .rposition(|candidate| same_card(candidate, card))
    }

    fn remove_card(&mut self, index: usize) -> Option<CardInfo> {
        if index >= self.hand.len() {
            return None;
        }
        self.hand_ids.remove(index);
        Some(self.hand.remove(index))
    }

    // Cards that stay in the same slot keep their identity so pending expiries still apply.
    pub(super) fn replace_hand(&mut self, hand: Vec<CardInfo>) {
        let previous = std::mem::take(&mut self.hand);
        let previous_ids = std::mem::take(&mut self.hand_ids);
        for (index, card) in hand.into_iter().enumerate() {
            let kept = previous
                .get(index)
                .is_some_and(|old| old.uid == card.uid && old.skill_id == card.skill_id);
            let card_id = if kept {
                previous_ids[index]
            } else {
                self.allocate_hand_id()
            };
            self.hand.push(card);
            self.hand_ids.push(card_id);
        }
    }

    fn push_hand(&mut self, card: CardInfo) {
        let card_id = self.allocate_hand_id();
        self.hand.push(card);
        self.hand_ids.push(card_id);
    }

    pub(super) fn insert_hand(&mut self, index: usize, card: CardInfo) -> Option<CardInstanceId> {
        if index > self.hand.len() {
            return None;
        }
        let card_id = self.allocate_hand_id();
        self.hand.insert(index, card);
        self.hand_ids.insert(index, card_id);
        Some(card_id)
    }

    fn allocate_hand_id(&mut self) -> CardInstanceId {
        let card_id = CardInstanceId(self.next_hand_id);
        self.next_hand_id = self.next_hand_id.wrapping_add(1).max(1);
        card_id
    }
}

fn composable_next(
    catalog: Option<crate::catalog::BattleCatalog>,
    left: &CardInfo,
    right: &CardInfo,
    rank_up: &HashMap<(i64, i32), i32>,
) -> Option<i32> {
    let owner_uid = left.uid?;
    let skill_id = left.skill_id?;
    if left.uid != right.uid
        || left.skill_id != right.skill_id
        || is_universal(skill_id)
        || is_special(owner_uid)
        || has_non_combine_enchant(left)
        || has_non_combine_enchant(right)
        || !permissions::permits(left, permissions::NOT_COMPOSE)
        || !permissions::permits(right, permissions::NOT_COMPOSE)
        || (left.card_type != Some(CardType::Skill3 as i32)
            && right.card_type != Some(CardType::Skill3 as i32)
            && catalog.is_some_and(|catalog| catalog.skill_is_big(skill_id)))
    {
        return None;
    }
    rank_up.get(&(owner_uid, skill_id)).copied()
}

pub(super) fn same_card(left: &CardInfo, right: &CardInfo) -> bool {
    left.uid == right.uid && left.skill_id == right.skill_id && left.temp_card == right.temp_card
}

fn is_universal(skill_id: i32) -> bool {
    super::UniversalCardSkill::try_from(skill_id).is_ok()
}

fn is_special(owner_uid: i64) -> bool {
    matches!(owner_uid, 0 | -99_999)
}

fn has_non_combine_enchant(card: &CardInfo) -> bool {
    has_enchant_type(card, EnchantedType::Lorenz)
}

pub(crate) fn has_enchant_type(card: &CardInfo, enchanted_type: EnchantedType) -> bool {
    card.enchants
        .iter()
        .any(|enchant| enchant.enchant_id == Some(enchanted_type.id()))
}

fn merge_enchants(
    catalog: Option<crate::catalog::BattleCatalog>,
    target: &mut Vec<CardEnchant>,
    source: &[CardEnchant],
) {
    for enchant in source {
        let Some(enchant_id) = enchant.enchant_id else {
            continue;
        };
        if !can_add_enchant(catalog, target, enchant_id) {
            continue;
        }
        let excluded = catalog
            .map(|catalog| catalog.card_enchant_excluded_ids(enchant_id))
            .unwrap_or_default();
        target.retain(|current| !current.enchant_id.is_some_and(|id| excluded.contains(&id)));
        if let Some(current) = target
            .iter_mut()
            .find(|current| current.enchant_id == Some(enchant_id))
        {
            current.duration = Some(match (current.duration, enchant.duration) {
                (Some(-1), _) | (_, Some(-1)) => -1,
                (left, right) => left.unwrap_or_default().max(right.unwrap_or_default()),
            });
        } else {
            target.push(enchant.clone());
        }
    }
}

fn can_add_enchant(
    catalog: Option<crate::catalog::BattleCatalog>,
    current: &[CardEnchant],
    target_id: i32,
) -> bool {
    let excluded = catalog
        .map(|catalog| catalog.card_enchant_excluded_ids(target_id))
        .unwrap_or_default();
    let mut check_limit = true;
    for enchant in current {
        let Some(id) = enchant.enchant_id else {
            continue;
        };
        if id == target_id {
            return true;
        }
        if catalog
            .map(|catalog| catalog.card_enchant_rejected_ids(id))
            .unwrap_or_default()
            .contains(&target_id)
        {
            return false;
        }
        if excluded.contains(&id) {
            check_limit = false;
        }
    }
    !check_limit || current.len() < 6
}

#[cfg(test)]
mod tests;
