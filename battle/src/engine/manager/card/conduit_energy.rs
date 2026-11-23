#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ConduitCardEnergy {
    pub power_id: i32,
    pub amount: i32,
    pub special: bool,
}

impl super::CardManager {
    pub(crate) fn conduit_skill_energy(&self, skill_id: i32) -> Option<ConduitCardEnergy> {
        let energy = self
            .deck
            .catalog()?
            .game_data()
            .skill_energy
            .get(skill_id)?;
        Some(ConduitCardEnergy {
            power_id: energy.energy_type,
            amount: energy.count,
            special: energy.is_special_card != 0,
        })
    }

    pub(crate) fn conduit_energy_cards(&self) -> Option<Vec<(usize, i32)>> {
        Some(
            self.hand()
                .iter()
                .enumerate()
                .filter_map(|(index, card)| {
                    let energy = self.conduit_skill_energy(card.skill_id?)?;
                    (!energy.special && energy.amount > 0).then_some((index, energy.amount))
                })
                .collect(),
        )
    }

    pub(super) fn consume_conduit_energy(&mut self, indices: &[usize]) -> bool {
        let Some(energy_cards) = self.conduit_energy_cards() else {
            return false;
        };
        if indices.windows(2).any(|pair| pair[0] >= pair[1])
            || indices
                .iter()
                .any(|index| !energy_cards.iter().any(|(candidate, _)| candidate == index))
        {
            return false;
        }
        for &index in indices.iter().rev() {
            self.deck
                .take_for_play(index)
                .expect("validated conduit energy card index remains present");
        }
        true
    }
}
