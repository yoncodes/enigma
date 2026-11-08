use sonettobuf::CardInfo;

use super::{CardManager, CardRefillStage};

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct ActionPhaseCards {
    completed_hand: Vec<CardInfo>,
    post_action_refilled: Option<Vec<CardInfo>>,
}

impl ActionPhaseCards {
    fn complete(&mut self, hand: &[CardInfo]) {
        self.completed_hand = hand.to_vec();
        self.post_action_refilled = None;
    }

    fn begin_post_action_refill(&mut self) {
        self.post_action_refilled = Some(Vec::new());
    }

    pub(super) fn record_refill(&mut self, stage: CardRefillStage, card: &CardInfo) {
        if stage == CardRefillStage::AfterActions
            && let Some(refilled) = &mut self.post_action_refilled
        {
            refilled.push(card.clone());
        }
    }

    fn clear(&mut self) {
        self.completed_hand.clear();
        self.post_action_refilled = None;
    }

    fn post_action_refill(&self) -> Option<(&[CardInfo], &[CardInfo])> {
        self.post_action_refilled
            .as_deref()
            .map(|refilled| (self.completed_hand.as_slice(), refilled))
    }
}

impl CardManager {
    pub(super) fn complete_action_phase(&mut self) {
        let hand = self.hand().to_vec();
        self.action_phase.complete(&hand);
    }

    pub(super) fn begin_post_action_refill(&mut self) {
        self.action_phase.begin_post_action_refill();
    }

    pub fn post_action_refill(&self) -> Option<(&[CardInfo], &[CardInfo])> {
        self.action_phase.post_action_refill()
    }

    pub(super) fn clear_action_phase(&mut self) {
        self.action_phase.clear();
    }
}
