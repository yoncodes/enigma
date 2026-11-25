use std::collections::HashSet;

use super::{CardInfo, CardManager, is_redealable, redeal_skill, start};

mod coverage;

impl CardManager {
    pub(crate) fn observed_redeal_draws(
        &self,
        observed: &[CardInfo],
        basics: &[CardInfo],
    ) -> Result<Vec<CardInfo>, String> {
        let ordinary_count = self.hand().len();
        let team_count = self.team_cards().len();
        if observed.len() != ordinary_count + team_count {
            return Err(format!(
                "capture has {} visible cards, replay has {} ordinary and {} team cards",
                observed.len(),
                ordinary_count,
                team_count,
            ));
        }
        let (observed, team_cards) = observed.split_at(ordinary_count);
        if let Some(index) = self
            .team_cards()
            .iter()
            .zip(team_cards)
            .position(|(current, observed)| !same_team_card(current, observed))
        {
            return Err(format!("capture changes preserved team card {}", index + 1));
        }

        let catalog = self.deck.catalog();
        let mut slots = Vec::new();
        for (index, (current, observed)) in self.hand().iter().zip(observed).enumerate() {
            if is_redealable(current, catalog) {
                let rank = catalog
                    .map(|catalog| catalog.card_skill_rank(current))
                    .unwrap_or_else(|| current.card_effect.unwrap_or_default());
                slots.push((rank, observed));
            } else if !same_card(current, observed) {
                return Err(format!(
                    "capture changes preserved ordinary card {}",
                    index + 1
                ));
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
            return Ok(draws);
        }
        let unreachable = "no legal draw sequence produces the captured hand";
        if !coverage {
            return Err(unreachable.into());
        }
        let draws = coverage::witness(&pile, basics, slots.len(), |card, index| {
            let (rank, observed) = slots[index];
            matches(card, rank, observed)
        })
        .ok_or(unreachable)?;
        reproduces(&draws, basics, &slots, matches)
            .then_some(draws)
            .ok_or_else(|| unreachable.into())
    }
}

fn same_card(a: &CardInfo, b: &CardInfo) -> bool {
    a.uid == b.uid && a.skill_id == b.skill_id
}

fn same_team_card(a: &CardInfo, b: &CardInfo) -> bool {
    fn normalized(card: &CardInfo) -> CardInfo {
        let mut card = card.clone();
        card.temp_card.get_or_insert(false);
        card.card_type.get_or_insert(0);
        card.status.get_or_insert(0);
        card.target_uid.get_or_insert(0);
        card.energy.get_or_insert(0);
        card.area_red_or_blue.get_or_insert(0);
        card.heat_id.get_or_insert(0);
        card
    }
    normalized(a) == normalized(b)
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
