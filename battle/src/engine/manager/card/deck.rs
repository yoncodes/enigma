use std::collections::{HashMap, HashSet};

use sonettobuf::{CardEnchant, CardInfo, card_info::CardType};

use super::{EnchantedType, precast_card, temp_card};

#[derive(Debug, Clone, PartialEq)]
pub struct CardDeck {
    hand: Vec<CardInfo>,
    // In-place card mutations preserve identity; structural replacements allocate a new one.
    hand_ids: Vec<CardInstanceId>,
    next_hand_id: u64,
    draw_pile: Vec<CardInfo>,
    discard_pile: Vec<CardInfo>,
    generated: Vec<CardInfo>,
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

    pub fn hand(&self) -> &[CardInfo] {
        &self.hand
    }

    pub(super) fn hand_mut(&mut self) -> &mut [CardInfo] {
        &mut self.hand
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
            remaining.remove(index);
        }
        for card in cards {
            self.consume_draw_card(card);
            self.push_hand(card.clone());
        }
        true
    }

    pub(super) fn recycle_draw_pile(&mut self) -> bool {
        if !self.can_recycle_draw_pile() {
            return false;
        }
        std::mem::swap(&mut self.draw_pile, &mut self.discard_pile);
        true
    }

    pub(super) fn can_recycle_draw_pile(&self) -> bool {
        self.draw_pile.is_empty() && !self.discard_pile.is_empty()
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
        while index + 1 < self.hand.len() {
            let left = &self.hand[index];
            let right = &self.hand[index + 1];
            let owner_uid = left.uid.unwrap_or_default();
            let Some(next_skill_id) = composable_next(left, right, rank_up) else {
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
            merge_enchants(&mut self.hand[index].enchants, &right.enchants);
            self.remove_card(index + 1);
            owners.push(owner_uid);
            index = index.saturating_sub(1);
        }
        owners
    }

    pub fn add_temp_card(&mut self, owner_uid: i64, skill_id: i32) -> CardInfo {
        let card = precast_card(owner_uid, skill_id);
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
        Some(
            indices
                .iter()
                .map(|index| {
                    merge_enchants(
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

    pub(super) fn expire_temporary(&mut self, card_ids: &[CardInstanceId]) {
        let card_ids = card_ids.iter().copied().collect::<HashSet<_>>();
        for index in (0..self.hand.len()).rev() {
            if card_ids.contains(&self.hand_ids[index]) {
                self.remove_card(index);
            }
        }
    }

    pub fn compose(&mut self, indices: &[usize], result: CardInfo) -> Option<CardInfo> {
        let mut indices = indices.to_vec();
        indices.sort_unstable();
        indices.dedup();
        if indices.len() < 2 || indices.iter().any(|index| *index >= self.hand.len()) {
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
        self.insert_hand(result_index, result.clone());
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
        let index = self.hand.iter().rposition(|candidate| {
            candidate.uid == card.uid
                && candidate.skill_id == card.skill_id
                && candidate.temp_card == card.temp_card
        })?;
        self.remove_card(index)
    }

    fn remove_card(&mut self, index: usize) -> Option<CardInfo> {
        if index >= self.hand.len() {
            return None;
        }
        self.hand_ids.remove(index);
        Some(self.hand.remove(index))
    }

    fn push_hand(&mut self, card: CardInfo) {
        let card_id = self.allocate_hand_id();
        self.hand.push(card);
        self.hand_ids.push(card_id);
    }

    fn insert_hand(&mut self, index: usize, card: CardInfo) {
        let card_id = self.allocate_hand_id();
        self.hand.insert(index, card);
        self.hand_ids.insert(index, card_id);
    }

    fn allocate_hand_id(&mut self) -> CardInstanceId {
        let card_id = CardInstanceId(self.next_hand_id);
        self.next_hand_id = self.next_hand_id.wrapping_add(1).max(1);
        card_id
    }
}

fn composable_next(
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
        || (left.card_type != Some(CardType::Skill3 as i32)
            && right.card_type != Some(CardType::Skill3 as i32)
            && crate::engine::skill::effect::catalog::configured_is_big_skill(skill_id))
    {
        return None;
    }
    rank_up.get(&(owner_uid, skill_id)).copied()
}

fn same_card(left: &CardInfo, right: &CardInfo) -> bool {
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

fn merge_enchants(target: &mut Vec<CardEnchant>, source: &[CardEnchant]) {
    for enchant in source {
        let Some(enchant_id) = enchant.enchant_id else {
            continue;
        };
        if !can_add_enchant(target, enchant_id) {
            continue;
        }
        let excluded = enchant_ids(enchant_id, |row| &row.exclude_types);
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

fn can_add_enchant(current: &[CardEnchant], target_id: i32) -> bool {
    let excluded = enchant_ids(target_id, |row| &row.exclude_types);
    let mut check_limit = true;
    for enchant in current {
        let Some(id) = enchant.enchant_id else {
            continue;
        };
        if id == target_id {
            return true;
        }
        if enchant_ids(id, |row| &row.reject_types).contains(&target_id) {
            return false;
        }
        if excluded.contains(&id) {
            check_limit = false;
        }
    }
    !check_limit || current.len() < 6
}

fn enchant_ids(
    enchant_id: i32,
    field: impl FnOnce(&config::card_enchant::CardEnchant) -> &str,
) -> Vec<i32> {
    config::try_get()
        .and_then(|db| db.card_enchant.get(enchant_id))
        .map(field)
        .into_iter()
        .flat_map(|raw| raw.split('#'))
        .filter_map(|id| id.parse().ok())
        .collect()
}

#[cfg(test)]
mod tests {
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

        let temp = deck.add_temp_card(10, 999);
        assert_eq!(temp.temp_card, Some(true));
        assert_eq!(deck.generated().len(), 1);

        let composed = deck.compose(&[0, 1], card(13, 400)).unwrap();
        assert_eq!(composed.skill_id, Some(400));

        let removed = deck.dissolve(0, Some(card(14, 500))).unwrap();
        assert_eq!(removed.skill_id, Some(999));
        assert_eq!(deck.hand().last().unwrap().skill_id, Some(500));
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
    fn temporary_lifetime_follows_a_card_when_the_hand_moves() {
        let mut deck = CardDeck::new(vec![card(10, 100), card(11, 200)]);
        let marked = deck.mark_temporary(&[0]).unwrap();

        assert!(deck.move_card(0, 1));
        deck.hand_mut()[1].energy = Some(5);
        deck.expire_temporary(&marked);

        assert_eq!(deck.hand(), &[card(11, 200)]);
    }

    #[test]
    fn adjacent_compose_respects_non_combine_enchants_and_merges_card_state() {
        crate::test_support::init_config();
        let mut blocked = card(10, 100);
        blocked.enchants.push(CardEnchant {
            enchant_id: Some(10_010),
            duration: Some(1),
            ..Default::default()
        });
        let mut deck = CardDeck::new(vec![blocked, card(10, 100)]);
        let rank_up = HashMap::from([((10, 100), 101)]);

        assert!(deck.compose_adjacent(&rank_up).is_empty());
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
        let mut deck = CardDeck::new(vec![left, right]);

        assert_eq!(deck.compose_adjacent(&rank_up), vec![10]);
        assert_eq!(deck.hand()[0].temp_card, Some(false));
        assert_eq!(deck.hand()[0].energy, Some(5));
        assert_eq!(deck.hand()[0].enchants[0].enchant_id, Some(10_006));
    }

    fn card(uid: i64, skill_id: i32) -> CardInfo {
        CardInfo {
            uid: Some(uid),
            skill_id: Some(skill_id),
            ..Default::default()
        }
    }
}
