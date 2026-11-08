use sonettobuf::CardInfo;

use super::{CardChangeKind, CardManager};

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct OpeningDeal {
    cards: Option<Vec<CardInfo>>,
}

impl OpeningDeal {
    fn begin(&mut self, cards: Vec<CardInfo>) {
        self.cards = Some(cards);
    }

    fn clear(&mut self) {
        self.cards = None;
    }

    fn record(&mut self, card: &CardInfo) {
        if let Some(cards) = &mut self.cards {
            cards.push(card.clone());
        }
    }

    fn record_change(
        &mut self,
        kind: CardChangeKind,
        before: &[CardInfo],
        after: &[CardInfo],
        added: Option<&CardInfo>,
        drawn: &[CardInfo],
    ) {
        if self.cards.is_none() {
            return;
        }
        match kind {
            CardChangeKind::SovereignInserted
            | CardChangeKind::GeneratedAdded
            | CardChangeKind::OwnedGeneratedAdded
            | CardChangeKind::HedoneAdded
            | CardChangeKind::GenericTemporaryAdded
            | CardChangeKind::TemporaryAdded
            | CardChangeKind::ConfiguredSkill3Added
            | CardChangeKind::HeroTemporaryAdded
            | CardChangeKind::CrystalAdded
            | CardChangeKind::PrecastAdded
            | CardChangeKind::UltimateAvailabilityChanged => {
                if let Some(card) = added {
                    self.record(card);
                }
            }
            CardChangeKind::UniversalAdded => {
                for card in after.iter().skip(before.len()) {
                    self.record(card);
                }
            }
            CardChangeKind::Drawn | CardChangeKind::Refilled => {
                for card in drawn {
                    self.record(card);
                }
            }
            _ => {}
        }
    }

    fn cards(&self) -> Option<&[CardInfo]> {
        self.cards.as_deref()
    }
}

impl CardManager {
    pub(crate) fn begin_opening_deal(&mut self) {
        self.opening_deal.begin(self.hand().to_vec());
    }

    pub fn opening_deal(&self) -> Option<&[CardInfo]> {
        self.opening_deal.cards()
    }

    pub(super) fn record_opening_deal_change(
        &mut self,
        kind: CardChangeKind,
        before: &[CardInfo],
        after: &[CardInfo],
        added: Option<&CardInfo>,
        drawn: &[CardInfo],
    ) {
        self.opening_deal
            .record_change(kind, before, after, added, drawn);
    }

    pub(super) fn clear_opening_deal(&mut self) {
        self.opening_deal.clear();
    }
}
