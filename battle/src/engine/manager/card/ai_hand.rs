use sonettobuf::CardInfo;

use super::{CardManager, deck::CardDeck};

impl CardManager {
    pub(crate) fn ai_hand(&self) -> &[CardInfo] {
        &self.ai_hand
    }

    pub(super) fn deal_ai_hand(&mut self, cards: Vec<CardInfo>) -> Vec<i64> {
        self.ai_hand.extend(cards);
        self.compose_ai_hand()
    }

    pub(super) fn queue_ai_hand(&mut self, queue: Vec<CardInfo>, from_hand: Vec<CardInfo>) {
        self.ai_hand_queued = from_hand;
        self.set_ai_queue(queue);
    }

    pub(super) fn settle_ai_hand(&mut self) -> Vec<i64> {
        for used in std::mem::take(&mut self.ai_hand_queued) {
            if let Some(index) = self
                .ai_hand
                .iter()
                .position(|card| super::deck::same_card(card, &used))
            {
                self.ai_hand.remove(index);
            }
        }
        self.compose_ai_hand()
    }

    pub(super) fn remove_ai_hand_owner(&mut self, owner_uid: i64) {
        self.ai_hand.retain(|card| card.uid != Some(owner_uid));
        self.ai_hand_queued
            .retain(|card| card.uid != Some(owner_uid));
    }

    fn compose_ai_hand(&mut self) -> Vec<i64> {
        let mut deck = CardDeck::new(std::mem::take(&mut self.ai_hand));
        if let Some(catalog) = self.deck.attached_catalog() {
            deck = deck.with_catalog(catalog);
        }
        let composed_owners = deck.compose_adjacent(&self.rank_up);
        self.ai_hand = deck.into_hand();
        composed_owners
    }
}
