impl super::CardManager {
    pub(crate) fn conduit_energy_cards(&self) -> Option<Vec<(usize, i32)>> {
        let game = self.deck.catalog()?.game_data();
        Some(
            self.hand()
                .iter()
                .enumerate()
                .filter_map(|(index, card)| {
                    let energy = game.skill_energy.get(card.skill_id?)?;
                    (energy.is_special_card == 0 && energy.count > 0)
                        .then_some((index, energy.count))
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
