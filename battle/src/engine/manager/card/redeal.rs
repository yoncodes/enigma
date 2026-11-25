use std::collections::HashSet;

use super::{CardInfo, CardManager, is_redealable, redeal_skill, start};

mod coverage;

impl CardManager {
    pub(crate) fn observed_redeal_draws(
        &self,
        observed: &[CardInfo],
        basics: &[CardInfo],
    ) -> Option<Vec<CardInfo>> {
        let (observed, team_cards) = observed.split_at_checked(self.hand().len())?;
        if team_cards.len() != self.team_cards().len()
            || !self
                .team_cards()
                .iter()
                .zip(team_cards)
                .all(|(a, b)| same_card(a, b))
        {
            return None;
        }

        let catalog = self.deck.catalog();
        let mut slots = Vec::new();
        for (current, observed) in self.hand().iter().zip(observed) {
            if is_redealable(current, catalog) {
                let rank = catalog
                    .map(|catalog| catalog.card_skill_rank(current))
                    .unwrap_or_else(|| current.card_effect.unwrap_or_default());
                slots.push((rank, observed));
            } else if !same_card(current, observed) {
                return None;
            }
        }
        let matches = |card: &CardInfo, rank, observed: &CardInfo| {
            card.uid == observed.uid
                && card.uid.zip(card.skill_id).is_some_and(|(owner, skill)| {
                    Some(redeal_skill(&self.rank_up, owner, skill, rank)) == observed.skill_id
                })
        };

        let mut seen = HashSet::new();
        let pile = self
            .draw_pile()
            .iter()
            .filter(|card| seen.insert((card.uid, card.skill_id)))
            .collect::<Vec<_>>();
        let coverage = !basics.is_empty() && basics.len() <= slots.len();
        let exact = slots
            .iter()
            .map(|(rank, observed)| {
                pile.iter()
                    .copied()
                    .find(|draw| matches(draw, *rank, observed))
                    .cloned()
            })
            .collect::<Option<Vec<_>>>();
        if let Some(draws) = exact
            && reproduces(&draws, basics, &slots, matches)
        {
            return Some(draws);
        }
        if !coverage {
            return None;
        }
        let draws = coverage::witness(&pile, basics, slots.len(), |card, index| {
            let (rank, observed) = slots[index];
            matches(card, rank, observed)
        })?;
        reproduces(&draws, basics, &slots, matches).then_some(draws)
    }
}

fn same_card(a: &CardInfo, b: &CardInfo) -> bool {
    a.uid == b.uid && a.skill_id == b.skill_id
}

fn reproduces(
    draws: &[CardInfo],
    basics: &[CardInfo],
    slots: &[(i32, &CardInfo)],
    matches: impl Fn(&CardInfo, i32, &CardInfo) -> bool,
) -> bool {
    let mut settled = draws.to_vec();
    start::preserve_basic_coverage(basics, &mut settled);
    settled
        .iter()
        .zip(slots)
        .all(|(card, (rank, observed))| matches(card, *rank, observed))
}
