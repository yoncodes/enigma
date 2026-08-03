use std::collections::{HashMap, HashSet};

pub mod ai;
pub mod change;
mod command;
pub mod deck;
pub mod draw;
pub mod enchant;
pub mod energy;
pub mod op;
pub mod pool;
pub mod start;
pub mod temp;
pub mod types;

pub use change::CardChange;
pub use command::{
    CARD_ENERGY_CLEAR_ORIGIN, CARD_PLAY_ORIGIN, CardActionQueue, CardAddCrystal, CardAddGenerated,
    CardAddPrecast, CardAddTemporary, CardAddUniversal, CardChangeKind, CardChangeToTemporary,
    CardChanges, CardCommand, CardCommandError, CardConsumeForEffect, CardDraw, CardEnchantHand,
    CardEnergyAllocation, CardEnergyChange, CardHandLimitChange, CardInvalidatePlayed,
    CardMarkTemporary, CardOpeningDraw, CardOwnerRemoval, CardPlay, CardQueueUse, CardRankChange,
    CardRankFailure, CardRankResult, CardRecordCastChannel, CardRedealKeepRanks, CardRefillOne,
    CardRefreshAiQueue, CardRemoveAiOwner, CardReplaceOwnerSkills, CardSetAiQueue,
    CardSetTeamCards, CardSetup, CardUseUniversal, HandCardRankUp, QueuedCardRankChange,
    QueuedCardRankUp, QueuedUseCard,
};
pub use deck::CardDeck;
use deck::CardInstanceId;
pub use enchant::EnchantedType;
pub use op::CardOpType;
pub use start::{hand_size, start_decks_from_fight};
pub use temp::{precast_card, selected_precast_card, temp_card};
pub use types::{CardPlayChoice, PlayedCard, UniversalCardSkill};

use sonettobuf::CardInfo;

#[derive(Debug, Clone, PartialEq)]
pub enum CastChannelState {
    Pending(Vec<CardInfo>),
    Resolved,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CardManager {
    deck: CardDeck,
    team_cards: Vec<CardInfo>,
    deck_num: i32,
    deck_capacity: i32,
    ai_queue: Vec<CardInfo>,
    cleaned_ai_owners: HashSet<i64>,
    played: Vec<PlayedCard>,
    refilled: Vec<CardInfo>,
    rank_up: HashMap<(i64, i32), i32>,
    queued_use_cards: Vec<QueuedUseCard>,
    cast_channels: HashMap<i64, CastChannelState>,
    played_history: HashMap<i64, Vec<i32>>,
    expiring_temporary: Vec<CardInstanceId>,
    hand_limit_bonus: i32,
    refill_floor: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CardRefill {
    pub drawn: Vec<CardInfo>,
    pub composed_owners: Vec<i64>,
}

impl CardManager {
    pub(crate) fn execute_command(
        &mut self,
        command: CardCommand,
    ) -> Result<CardChanges, CardCommandError> {
        command::execute(self, command)
    }

    pub fn new(hand: Vec<CardInfo>) -> Self {
        Self {
            deck: CardDeck::new(hand),
            team_cards: Vec::new(),
            deck_num: 0,
            deck_capacity: 0,
            ai_queue: Vec::new(),
            cleaned_ai_owners: HashSet::new(),
            played: Vec::new(),
            refilled: Vec::new(),
            rank_up: HashMap::new(),
            queued_use_cards: Vec::new(),
            cast_channels: HashMap::new(),
            played_history: HashMap::new(),
            expiring_temporary: Vec::new(),
            hand_limit_bonus: 0,
            refill_floor: 0,
        }
    }

    pub fn with_draw_pile(hand: Vec<CardInfo>, draw_pile: Vec<CardInfo>) -> Self {
        Self {
            deck: CardDeck::with_draw_pile(hand, draw_pile),
            team_cards: Vec::new(),
            deck_num: 0,
            deck_capacity: 0,
            ai_queue: Vec::new(),
            cleaned_ai_owners: HashSet::new(),
            played: Vec::new(),
            refilled: Vec::new(),
            rank_up: HashMap::new(),
            queued_use_cards: Vec::new(),
            cast_channels: HashMap::new(),
            played_history: HashMap::new(),
            expiring_temporary: Vec::new(),
            hand_limit_bonus: 0,
            refill_floor: 0,
        }
    }

    pub fn hand(&self) -> &[CardInfo] {
        self.deck.hand()
    }

    pub fn plan_effect_consumption(&self, owner_uid: i64) -> Vec<(usize, i32)> {
        self.hand()
            .iter()
            .enumerate()
            .filter(|(_, card)| self.is_registered_skill_card(owner_uid, card))
            .map(|(index, card)| (index, crate::engine::entity::skill::card_skill_rank(card)))
            .collect()
    }

    pub fn normal_hand_len(&self) -> usize {
        self.hand()
            .iter()
            .filter(|card| !card.temp_card.unwrap_or_default())
            .count()
    }

    pub fn hand_limit_bonus(&self) -> i32 {
        self.hand_limit_bonus
    }

    pub fn refill_floor(&self) -> usize {
        self.refill_floor
    }

    pub fn refill_consumes_deck(&self) -> bool {
        self.refill_floor == 0
    }

    pub(crate) fn preserve_refill_floor(&mut self) {
        self.refill_floor = self.normal_hand_len();
    }

    pub(crate) fn add_hand_limit_bonus(&mut self, delta: i32) {
        self.hand_limit_bonus = self.hand_limit_bonus.saturating_add(delta);
    }

    pub(crate) fn clear_hand_limit_bonus(&mut self) {
        self.hand_limit_bonus = 0;
    }

    pub fn team_cards(&self) -> &[CardInfo] {
        &self.team_cards
    }

    pub(crate) fn visible_card(&self, index: usize) -> Option<&CardInfo> {
        self.hand()
            .get(index)
            .or_else(|| self.team_cards.get(index.checked_sub(self.hand().len())?))
    }

    fn set_team_cards(&mut self, cards: Vec<CardInfo>) {
        self.team_cards = cards;
    }

    pub fn ai_queue(&self) -> &[CardInfo] {
        &self.ai_queue
    }

    fn set_ai_queue(&mut self, cards: Vec<CardInfo>) {
        for owner_uid in cards.iter().filter_map(|card| card.uid) {
            self.cleaned_ai_owners.remove(&owner_uid);
        }
        self.ai_queue = cards;
    }

    fn refresh_ai_queue(&mut self, cards: Vec<CardInfo>) -> Vec<i64> {
        let composed_owners = cards
            .iter()
            .filter_map(|card| Some((card.uid?, card.skill_id?)))
            .flat_map(|(owner_uid, skill_id)| {
                let depth = self.rank_depth(owner_uid, skill_id);
                std::iter::repeat_n(owner_uid, (1_usize << depth).saturating_sub(1))
            })
            .collect();
        self.set_ai_queue(cards);
        composed_owners
    }

    fn rank_depth(&self, owner_uid: i64, skill_id: i32) -> u32 {
        let mut current = skill_id;
        let mut depth = 0;
        while let Some(lower) = self.rank_up.iter().find_map(|(&(uid, lower), &higher)| {
            (uid == owner_uid && higher == current).then_some(lower)
        }) {
            current = lower;
            depth += 1;
        }
        depth
    }

    fn remove_ai_owner_cards(&mut self, owner_uid: i64) -> Option<Vec<i64>> {
        if !self.cleaned_ai_owners.insert(owner_uid) {
            return None;
        }
        self.ai_queue.retain(|card| card.uid != Some(owner_uid));
        let mut deck = CardDeck::new(std::mem::take(&mut self.ai_queue));
        let composed_owners = deck.compose_adjacent(&self.rank_up);
        self.ai_queue = deck.into_hand();
        Some(composed_owners)
    }

    pub(crate) fn hand_mut(&mut self) -> &mut [CardInfo] {
        self.deck.hand_mut()
    }

    pub fn reset(&mut self, hand: Vec<CardInfo>, deck_num: i32) {
        self.reset_with_draw_pile(hand, Vec::new(), deck_num);
    }

    pub fn reset_with_draw_pile(
        &mut self,
        hand: Vec<CardInfo>,
        draw_pile: Vec<CardInfo>,
        deck_num: i32,
    ) {
        self.deck = CardDeck::with_draw_pile(hand, draw_pile);
        self.team_cards.clear();
        self.deck_num = deck_num;
        self.deck_capacity = deck_num;
        self.played.clear();
        self.refilled.clear();
        self.queued_use_cards.clear();
        self.cast_channels.clear();
        self.played_history.clear();
        self.expiring_temporary.clear();
        self.refill_floor = 0;
    }

    pub fn seed(&mut self, fight: &sonettobuf::Fight) {
        for entity in crate::engine::manager::entities(fight) {
            self.register_skill_groups(entity);
        }
    }

    pub(super) fn begin_round(&mut self) {
        self.played.clear();
        self.refilled.clear();
        self.queued_use_cards.clear();
    }

    pub fn queued_use_cards(&self) -> &[QueuedUseCard] {
        &self.queued_use_cards
    }

    pub(crate) fn queue_use_card(&mut self, card: QueuedUseCard) {
        self.queued_use_cards.push(card);
    }

    pub(crate) fn record_cast_channel(&mut self, buff_uid: i64, cards: Vec<CardInfo>) {
        self.cast_channels
            .insert(buff_uid, CastChannelState::Pending(cards));
    }

    pub fn cast_channel(&self, buff_uid: i64) -> Option<&CastChannelState> {
        self.cast_channels.get(&buff_uid)
    }

    pub(crate) fn resolve_cast_channel(&mut self, buff_uid: i64) -> bool {
        let Some(state) = self.cast_channels.get_mut(&buff_uid) else {
            return false;
        };
        *state = CastChannelState::Resolved;
        true
    }

    pub(crate) fn remove_cast_channel(&mut self, buff_uid: i64) -> bool {
        self.cast_channels.remove(&buff_uid).is_some()
    }

    fn replace_owner_skills(
        &mut self,
        owner_uid: i64,
        base_group1: &[i32],
        base_group2: &[i32],
        replacement_group1: &[i32],
        replacement_group2: &[i32],
    ) {
        self.deck.replace_owner_skills(
            owner_uid,
            base_group1,
            base_group2,
            replacement_group1,
            replacement_group2,
        );
        self.register_skill_group(owner_uid, replacement_group1);
        self.register_skill_group(owner_uid, replacement_group2);
    }

    pub fn deck_num(&self) -> i32 {
        self.deck_num
    }

    pub fn set_deck_num(&mut self, deck_num: i32) {
        self.deck_num = deck_num;
    }

    pub(crate) fn recycle_draw_pile(&mut self) -> Option<i32> {
        self.deck.recycle_draw_pile().then(|| {
            self.deck_num = self.deck_capacity;
            self.deck_num
        })
    }

    pub(crate) fn can_recycle_draw_pile(&self) -> bool {
        self.deck.can_recycle_draw_pile()
    }

    pub fn into_hand(self) -> Vec<CardInfo> {
        self.deck.into_hand()
    }

    pub fn played(&self) -> &[PlayedCard] {
        &self.played
    }

    pub fn total_played_rank(&self) -> i32 {
        self.played
            .iter()
            .map(|played| crate::engine::entity::skill::card_skill_rank(&played.card))
            .fold(0, i32::saturating_add)
    }

    pub fn resolving_ranks(&self) -> impl Iterator<Item = i32> + '_ {
        self.played
            .iter()
            .map(|played| crate::engine::entity::skill::card_skill_rank(&played.card))
            .chain(
                self.queued_use_cards
                    .iter()
                    .map(|queued| crate::engine::entity::skill::card_skill_rank(&queued.card)),
            )
    }

    pub fn total_resolving_rank(&self) -> i32 {
        self.resolving_ranks().fold(0, i32::saturating_add)
    }

    pub fn played_skill_counts(&self, owner_uid: i64) -> Vec<(i32, i32)> {
        let mut counts = Vec::<(i32, i32)>::new();
        for skill_id in self.played_history.get(&owner_uid).into_iter().flatten() {
            if let Some((_, count)) = counts.iter_mut().find(|(id, _)| id == skill_id) {
                *count += 1;
            } else {
                counts.push((*skill_id, 1));
            }
        }
        counts
    }

    pub(crate) fn invalidate_played(&mut self, card_index: i32, restore: bool) -> Option<CardInfo> {
        let index = self
            .played
            .iter()
            .position(|played| played.card_index == card_index)?;
        let card = self.played.remove(index).card;
        if restore {
            self.deck.add_to_hand(card.clone());
        }
        Some(card)
    }

    pub fn resolve_played_ranks(&mut self) -> Vec<CardRankChange> {
        self.played
            .iter_mut()
            .filter_map(|played| {
                played.rank_change_pending.then(|| {
                    let old_rank = crate::engine::entity::skill::skill_rank(
                        played.card.skill_id.unwrap_or_default(),
                    );
                    played.rank_change_pending = false;
                    played.card.skill_id = Some(played.skill_id);
                    played.card.card_type =
                        Some(crate::engine::entity::skill::skill_rank(played.skill_id));
                    let mut resolved_card = played.card.clone();
                    resolved_card.uid = Some(played.caster_uid);
                    CardRankChange {
                        owner_uid: played.caster_uid,
                        card_index: played.card_index,
                        card: resolved_card,
                        rewritten: played.rewritten,
                        rank_delta: crate::engine::entity::skill::skill_rank(played.skill_id)
                            - old_rank,
                    }
                })
            })
            .collect()
    }

    pub fn refilled(&self) -> &[CardInfo] {
        &self.refilled
    }

    pub(crate) fn record_refill(&mut self, card: CardInfo) {
        self.refilled.push(card);
    }

    pub fn rank_up_played(&mut self, card_index: i32, levels: i32, rewritten: bool) -> Option<i32> {
        let played = self
            .played
            .iter()
            .find(|played| played.card_index == card_index)?;
        let owner_uid = played.caster_uid;
        let mut skill_id = played.skill_id;
        for _ in 0..levels.max(0) {
            let Some(next) = self.rank_up.get(&(owner_uid, skill_id)).copied() else {
                break;
            };
            skill_id = next;
        }
        let played = self
            .played
            .iter_mut()
            .find(|played| played.card_index == card_index)?;
        played.rank_change_pending |= played.skill_id != skill_id;
        played.rewritten |= rewritten && played.skill_id != skill_id;
        played.skill_id = skill_id;
        Some(skill_id)
    }

    pub(crate) fn change_played_rank(
        &mut self,
        card_index: i32,
        levels: i32,
    ) -> Result<CardRankChange, CardRankFailure> {
        let Some(played_index) = self
            .played
            .iter()
            .position(|played| played.card_index == card_index)
        else {
            return Err(CardRankFailure {
                owner_uid: 0,
                card_index,
                card: Box::default(),
                requested_delta: levels,
            });
        };
        let owner_uid = self.played[played_index].card.uid.unwrap_or_default();
        let original = self.played[played_index]
            .card
            .skill_id
            .unwrap_or(self.played[played_index].skill_id);
        let mut skill_id = original;
        for _ in 0..levels.unsigned_abs() {
            let next = if levels > 0 {
                self.rank_up.get(&(owner_uid, skill_id)).copied()
            } else {
                let current_rank = crate::engine::entity::skill::skill_rank(skill_id);
                if current_rank > 1 {
                    self.rank_up.iter().find_map(|((uid, lower), higher)| {
                        (*uid == owner_uid
                            && *higher == skill_id
                            && crate::engine::entity::skill::skill_rank(*lower) == current_rank - 1)
                            .then_some(*lower)
                    })
                } else {
                    None
                }
            };
            let Some(next) = next else {
                break;
            };
            skill_id = next;
        }
        if skill_id == original {
            let played = &self.played[played_index];
            return Err(CardRankFailure {
                owner_uid,
                card_index,
                card: Box::new(played.card.clone()),
                requested_delta: levels,
            });
        }
        let old_rank = crate::engine::entity::skill::skill_rank(original);
        let new_rank = crate::engine::entity::skill::skill_rank(skill_id);
        let played = &mut self.played[played_index];
        played.skill_id = skill_id;
        played.card.skill_id = Some(skill_id);
        played.card.card_type = Some(new_rank);
        Ok(CardRankChange {
            owner_uid,
            card_index,
            card: played.card.clone(),
            rewritten: false,
            rank_delta: new_rank - old_rank,
        })
    }

    fn rank_up_hand(
        &mut self,
        owner_uid: i64,
        hand_index: usize,
    ) -> Result<CardRankChange, CardCommandError> {
        let card = self
            .deck
            .hand()
            .get(hand_index)
            .filter(|card| card.uid == Some(owner_uid) && !card.temp_card.unwrap_or_default())
            .ok_or(CardCommandError::InvalidCommand)?;
        let skill_id = card.skill_id.ok_or(CardCommandError::InvalidCommand)?;
        let next_skill_id = self
            .rank_up
            .get(&(owner_uid, skill_id))
            .copied()
            .ok_or(CardCommandError::InvalidCommand)?;
        let rank_delta = crate::engine::entity::skill::skill_rank(next_skill_id)
            - crate::engine::entity::skill::skill_rank(skill_id);
        if rank_delta <= 0 {
            return Err(CardCommandError::InvalidCommand);
        }
        let card = self
            .deck
            .hand_mut()
            .get_mut(hand_index)
            .ok_or(CardCommandError::InvalidCommand)?;
        card.skill_id = Some(next_skill_id);
        Ok(CardRankChange {
            owner_uid,
            card_index: i32::try_from(hand_index)
                .ok()
                .and_then(|index| index.checked_add(1))
                .ok_or(CardCommandError::InvalidCommand)?,
            card: card.clone(),
            rewritten: false,
            rank_delta,
        })
    }

    pub fn rank_up_played_after(&mut self, card_index: i32, count: i32, levels: i32) {
        let indices = self
            .played
            .iter()
            .enumerate()
            .filter(|(_, played)| played.card_index > card_index)
            .take(count.max(0) as usize)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        for index in indices {
            let played = &self.played[index];
            let owner_uid = played.caster_uid;
            let mut skill_id = played.skill_id;
            for _ in 0..levels.max(0) {
                let Some(next) = self.rank_up.get(&(owner_uid, skill_id)).copied() else {
                    break;
                };
                skill_id = next;
            }
            let played = &mut self.played[index];
            played.rank_change_pending |= played.skill_id != skill_id;
            played.skill_id = skill_id;
        }
    }

    pub fn generated(&self) -> &[CardInfo] {
        self.deck.generated()
    }

    pub fn draw_pile(&self) -> &[CardInfo] {
        self.deck.draw_pile()
    }

    pub(crate) fn redealable_cards(&self) -> impl Iterator<Item = &CardInfo> {
        self.hand().iter().filter(|card| is_redealable(card))
    }

    pub(crate) fn redealable_count(&self) -> usize {
        self.redealable_cards().count()
    }

    fn add_universal_cards(&mut self, count: i32, skill: UniversalCardSkill) {
        for _ in 0..count {
            self.deck.add_to_hand(CardInfo {
                uid: Some(0),
                skill_id: Some(skill.id()),
                temp_card: Some(false),
                ..Default::default()
            });
        }
    }

    fn use_universal(&mut self, universal_index: usize, target_index: usize) -> Option<i64> {
        if universal_index == target_index {
            return None;
        }
        let universal = self.hand().get(universal_index)?;
        let target = self.hand().get(target_index)?;
        let owner_uid = target.uid.filter(|uid| *uid != 0)?;
        let target_skill = target.skill_id?;
        if universal.skill_id != Some(UniversalCardSkill::RankOne.id())
            || crate::engine::entity::skill::card_skill_rank(target) != 1
            || deck::has_enchant_type(target, EnchantedType::Lorenz)
        {
            return None;
        }
        let next_skill = self.rank_up.get(&(owner_uid, target_skill)).copied()?;
        let mut result = target.clone();
        result.skill_id = Some(next_skill);
        self.deck
            .combine_universal(universal_index, target_index, result)?;
        Some(owner_uid)
    }

    fn redeal_keep_ranks(&mut self, replacements: Vec<CardInfo>) {
        let mut replacements = replacements.into_iter();
        for card in self
            .deck
            .hand_mut()
            .iter_mut()
            .filter(|card| is_redealable(card))
        {
            let Some(mut replacement) = replacements.next() else {
                break;
            };
            let rank = crate::engine::entity::skill::card_skill_rank(card);
            let owner_uid = replacement.uid.unwrap_or_default();
            let mut skill_id = replacement.skill_id.unwrap_or_default();
            for _ in 1..rank {
                let Some(next) = self.rank_up.get(&(owner_uid, skill_id)).copied() else {
                    break;
                };
                skill_id = next;
            }
            replacement.skill_id = Some(skill_id);
            *card = replacement;
        }
    }

    pub(crate) fn consume_draw_card(&mut self, card: &CardInfo) -> bool {
        self.deck.consume_draw_card(card)
    }

    pub(crate) fn deal_opening_cards(&mut self, cards: &[CardInfo], deck_cost: i32) -> bool {
        if deck_cost < 0
            || deck_cost as usize > cards.len()
            || self.deck_num < deck_cost
            || !self.deck.deal_from_draw_pile(cards)
        {
            return false;
        }
        self.deck_num -= deck_cost;
        true
    }

    pub fn move_card(&mut self, from_index: usize, to_index: usize) -> bool {
        self.deck.move_card(from_index, to_index)
    }

    pub fn play_card(
        &mut self,
        card_index: usize,
        target_uid: Option<i64>,
        chosen_skill_id: Option<i32>,
        choice: Option<CardPlayChoice>,
    ) -> Option<PlayedCard> {
        self.take_for_play(card_index, target_uid, chosen_skill_id, choice, None)
    }

    pub fn play_card_with_record(
        &mut self,
        card_index: usize,
        target_uid: Option<i64>,
        chosen_skill_id: Option<i32>,
        choice: Option<CardPlayChoice>,
        recorded_skill: Option<crate::engine::skill::action::SkillRequest>,
    ) -> Option<PlayedCard> {
        self.take_for_play(
            card_index,
            target_uid,
            chosen_skill_id,
            choice,
            recorded_skill,
        )
    }

    pub fn draw(&mut self, count: usize) -> Vec<CardInfo> {
        self.deck.draw(count)
    }

    pub fn refill_to(
        &mut self,
        hand_size: usize,
        mut draw: impl FnMut() -> Option<CardInfo>,
    ) -> CardRefill {
        let mut refill = CardRefill::default();
        while self.normal_hand_len() < hand_size {
            let Some(card) = draw() else { break };
            self.deck.add_to_hand(card.clone());
            refill.drawn.push(card);
            refill
                .composed_owners
                .extend(self.deck.compose_adjacent(&self.rank_up));
        }
        refill
    }

    pub fn compose_adjacent(&mut self) -> Vec<i64> {
        self.deck.compose_adjacent(&self.rank_up)
    }

    pub fn add_basic_card_energy(&mut self, delta: i32, count: i32) {
        for card in self.deck.hand_mut().iter_mut().filter(|card| {
            !card.temp_card.unwrap_or_default()
                && (1..=3).contains(&crate::engine::entity::skill::card_skill_rank(card))
        }) {
            *card.energy.get_or_insert(0) += delta * count;
        }
    }

    pub fn clear_energy(&mut self) {
        for card in self.deck.hand_mut() {
            card.energy = Some(0);
        }
    }

    fn register_skill_groups(&mut self, entity: &sonettobuf::FightEntityInfo) {
        let Some(owner_uid) = entity.uid else { return };
        self.register_skill_group(owner_uid, &entity.skill_group1);
        self.register_skill_group(owner_uid, &entity.skill_group2);
    }

    fn register_skill_group(&mut self, owner_uid: i64, skills: &[i32]) {
        for pair in skills.windows(2) {
            self.rank_up.insert((owner_uid, pair[0]), pair[1]);
        }
    }

    fn is_registered_skill_card(&self, owner_uid: i64, card: &CardInfo) -> bool {
        let Some(skill_id) = card.skill_id else {
            return false;
        };
        card.uid == Some(owner_uid)
            && self.rank_up.iter().any(|(&(uid, lower), &higher)| {
                uid == owner_uid && (lower == skill_id || higher == skill_id)
            })
    }

    fn consume_for_effect(&mut self, owner_uid: i64, indices: &[usize]) -> bool {
        if indices.windows(2).any(|pair| pair[0] >= pair[1])
            || indices.iter().any(|&index| {
                self.hand()
                    .get(index)
                    .is_none_or(|card| !self.is_registered_skill_card(owner_uid, card))
            })
        {
            return false;
        }
        for &index in indices.iter().rev() {
            self.deck
                .take_for_play(index)
                .expect("validated card index remains present");
        }
        true
    }

    pub fn add_to_hand(&mut self, card: CardInfo) -> CardInfo {
        self.add_to_hand_for(card.uid.unwrap_or_default(), card)
    }

    pub fn add_to_hand_for(&mut self, _target_uid: i64, card: CardInfo) -> CardInfo {
        self.deck.add_to_hand(card)
    }

    pub fn add_temp_card(&mut self, skill_id: i32) -> CardInfo {
        self.add_temp_card_for(0, skill_id, 0, 1)
    }

    pub fn add_temp_card_for(
        &mut self,
        target_uid: i64,
        skill_id: i32,
        _reserve_id: i64,
        _team_type: i32,
    ) -> CardInfo {
        self.deck.add_temp_card(target_uid, skill_id)
    }

    pub fn change_to_temp_card(&mut self, index: usize, skill_id: i32) -> Option<CardInfo> {
        let target_uid = self.deck.hand().get(index)?.uid.unwrap_or_default();
        self.change_to_temp_card_for(index, skill_id, target_uid, skill_id.to_string(), 1)
    }

    pub fn change_to_temp_card_for(
        &mut self,
        index: usize,
        skill_id: i32,
        _target_uid: i64,
        _reserve_str: String,
        _team_type: i32,
    ) -> Option<CardInfo> {
        self.deck.change_to_temp_card(index, skill_id)
    }

    fn enchant_hand(
        &mut self,
        indices: &[usize],
        enchant: EnchantedType,
        duration: i32,
    ) -> Option<Vec<CardInfo>> {
        self.deck.enchant(indices, enchant, duration)
    }

    fn mark_hand_temporary(&mut self, indices: &[usize]) -> bool {
        let Some(card_ids) = self.deck.mark_temporary(indices) else {
            return false;
        };
        self.expiring_temporary = card_ids;
        true
    }

    fn expire_temporary(&mut self) {
        self.deck
            .expire_temporary(&std::mem::take(&mut self.expiring_temporary));
    }

    pub fn compose(&mut self, indices: &[usize], result: CardInfo) -> Option<CardInfo> {
        self.deck.compose(indices, result)
    }

    pub fn dissolve(&mut self, index: usize, replacement: Option<CardInfo>) -> Option<CardInfo> {
        self.deck.dissolve(index, replacement)
    }

    fn take_for_play(
        &mut self,
        card_index: usize,
        target_uid: Option<i64>,
        chosen_skill_id: Option<i32>,
        choice: Option<CardPlayChoice>,
        recorded_skill: Option<crate::engine::skill::action::SkillRequest>,
    ) -> Option<PlayedCard> {
        let recorded_skill = recorded_skill.filter(|recorded| {
            self.deck.hand().iter().any(|card| {
                card.uid == Some(recorded.source_uid) && card.skill_id == Some(recorded.skill_id)
            })
        });
        let has_choice = choice.is_some();
        let choice_matches_index = choice.as_ref().is_some_and(|choice| {
            self.deck.hand().get(card_index).is_some_and(|card| {
                card.uid == choice.source.uid
                    && card.skill_id == choice.source.skill_id
                    && card.temp_card == choice.source.temp_card
            })
        });
        let source = if choice_matches_index {
            self.deck.take_for_play(card_index)
        } else if let Some(index) = choice.as_ref().and_then(|choice| {
            self.team_cards.iter().rposition(|candidate| {
                candidate.uid == choice.source.uid
                    && candidate.skill_id == choice.source.skill_id
                    && candidate.temp_card == choice.source.temp_card
            })
        }) {
            Some(self.team_cards.remove(index))
        } else {
            choice
                .as_ref()
                .and_then(|choice| self.deck.take_matching(&choice.source))
                .or_else(|| self.deck.take_for_play(card_index))
                .or_else(|| {
                    card_index
                        .checked_sub(self.deck.hand().len())
                        .filter(|index| *index < self.team_cards.len())
                        .map(|index| self.team_cards.remove(index))
                })
        }?;
        let resolved_skill_id = choice.as_ref().and_then(|choice| choice.played.skill_id);
        let caster_uid = choice
            .as_ref()
            .and_then(|choice| choice.played.uid)
            .filter(|uid| *uid != 0)
            .or(source.uid)
            .unwrap_or_default();
        let skill_id = if has_choice {
            resolved_skill_id
        } else {
            chosen_skill_id.or(source.skill_id)
        }
        .unwrap_or_default();
        if skill_id == 0 {
            return None;
        }
        let action_index = self.played.len() as i32 + 1;

        let played = PlayedCard {
            card: source,
            caster_uid,
            card_index: action_index,
            skill_id,
            rank_change_pending: false,
            rewritten: false,
            target_uid,
            recorded_skill,
        };
        self.played.push(played.clone());
        self.played_history
            .entry(played.caster_uid)
            .or_default()
            .push(played.skill_id);
        Some(played)
    }
}

fn is_redealable(card: &CardInfo) -> bool {
    !card.temp_card.unwrap_or_default()
        && card.uid.unwrap_or_default() != 0
        && !card
            .skill_id
            .is_some_and(crate::engine::skill::effect::catalog::configured_is_big_skill)
}

#[cfg(test)]
mod test;
