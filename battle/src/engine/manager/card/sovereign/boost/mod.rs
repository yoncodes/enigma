use super::{CardCommandError, CardManager, MAX_ENHANCEMENT_LEVEL, state};
use crate::engine::manager::card::{CardInitialSovereignBoost, SovereignBoostChange};

const ENHANCEMENTS_PER_POSITION: usize = 2;

impl CardManager {
    pub(in crate::engine::manager::card) fn initial_sovereign_boost(
        &mut self,
        boost: CardInitialSovereignBoost,
    ) -> Result<SovereignBoostChange, CardCommandError> {
        if boost.owner_uid == 0 || boost.team_type != 1 || boost.distance != 8 {
            return Err(CardCommandError::InvalidCommand);
        }
        let card = self
            .hand()
            .get(boost.index)
            .filter(|card| card.uid == Some(boost.owner_uid))
            .cloned()
            .ok_or(CardCommandError::InvalidCommand)?;
        let state = state(&card)
            .map_err(|_| CardCommandError::InvalidCommand)?
            .ok_or(CardCommandError::InvalidCommand)?;
        if !state.lock || state.strengthen.levels() != [0; 4] {
            return Err(CardCommandError::InvalidCommand);
        }
        self.shift_sovereign(boost.index, card, state, boost.distance, boost.team_type)
    }

    /// Moves the held sovereign card right and enhances it twice per position asked for.
    pub(in crate::engine::manager::card) fn shift_owner_sovereign(
        &mut self,
        owner_uid: i64,
        distance: usize,
        team_type: i32,
    ) -> Result<SovereignBoostChange, CardCommandError> {
        if owner_uid == 0 || team_type != 1 || distance == 0 {
            return Err(CardCommandError::InvalidCommand);
        }
        let (index, card) = self
            .hand()
            .iter()
            .enumerate()
            .find(|(_, card)| card.uid == Some(owner_uid) && Self::is_sovereign(card))
            .map(|(index, card)| (index, card.clone()))
            .ok_or(CardCommandError::InvalidCommand)?;
        let state = state(&card)
            .map_err(|_| CardCommandError::InvalidCommand)?
            .ok_or(CardCommandError::InvalidCommand)?;
        self.shift_sovereign(index, card, state, distance, team_type)
    }

    fn shift_sovereign(
        &mut self,
        index: usize,
        mut card: sonettobuf::CardInfo,
        mut state: super::SovereignState,
        distance: usize,
        team_type: i32,
    ) -> Result<SovereignBoostChange, CardCommandError> {
        for _ in 0..distance * ENHANCEMENTS_PER_POSITION {
            // Fewest stacks first; equal stacks fill in type order.
            let Some(level) = state
                .strengthen
                .levels_mut()
                .into_iter()
                .filter(|level| **level < MAX_ENHANCEMENT_LEVEL)
                .min_by_key(|level| **level)
            else {
                break;
            };
            *level += 1;
        }
        let value = serde_json::to_string(&state).map_err(|_| CardCommandError::InvalidCommand)?;
        card.card_dataes
            .iter_mut()
            .find(|data| data.key == Some(sonettobuf::card_data::CardDataKey::Unnamed as i32))
            .ok_or(CardCommandError::InvalidCommand)?
            .value = Some(value);
        let to_index = index.saturating_sub(distance);
        if index != to_index && !self.deck.move_card(index, to_index) {
            return Err(CardCommandError::InvalidCommand);
        }
        self.deck.hand_mut()[to_index] = card.clone();
        Ok(SovereignBoostChange {
            from_index: index,
            to_index,
            card,
            team_type,
        })
    }
}

#[cfg(test)]
mod tests;
