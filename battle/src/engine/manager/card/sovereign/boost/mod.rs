use super::{CardCommandError, CardManager, SovereignState, Strengthen, payload};
use crate::engine::manager::card::{CardInitialSovereignBoost, SovereignBoostChange};

impl CardManager {
    pub(in crate::engine::manager::card) fn initial_sovereign_boost(
        &mut self,
        boost: CardInitialSovereignBoost,
    ) -> Result<SovereignBoostChange, CardCommandError> {
        if boost.owner_uid == 0 || boost.team_type != 1 || boost.distance != 8 {
            return Err(CardCommandError::InvalidCommand);
        }
        let mut card = self
            .hand()
            .get(boost.index)
            .filter(|card| card.uid == Some(boost.owner_uid))
            .cloned()
            .ok_or(CardCommandError::InvalidCommand)?;
        let mut state: SovereignState = {
            let mut entries = payload(&card);
            let data = entries.next().ok_or(CardCommandError::InvalidCommand)?;
            if entries.next().is_some() {
                return Err(CardCommandError::InvalidCommand);
            }
            serde_json::from_str(
                data.value
                    .as_deref()
                    .ok_or(CardCommandError::InvalidCommand)?,
            )
            .map_err(|_| CardCommandError::InvalidCommand)?
        };
        if !state.lock
            || [
                state.strengthen.first,
                state.strengthen.second,
                state.strengthen.third,
                state.strengthen.fourth,
            ] != [0; 4]
        {
            return Err(CardCommandError::InvalidCommand);
        }
        state.strengthen = Strengthen {
            first: 4,
            second: 4,
            third: 4,
            fourth: 4,
        };
        let value = serde_json::to_string(&state).map_err(|_| CardCommandError::InvalidCommand)?;
        card.card_dataes
            .iter_mut()
            .find(|data| data.key == Some(sonettobuf::card_data::CardDataKey::Unnamed as i32))
            .ok_or(CardCommandError::InvalidCommand)?
            .value = Some(value);
        let to_index = boost.index.saturating_sub(boost.distance);
        if boost.index != to_index && !self.deck.move_card(boost.index, to_index) {
            return Err(CardCommandError::InvalidCommand);
        }
        self.deck.hand_mut()[to_index] = card.clone();
        Ok(SovereignBoostChange {
            from_index: boost.index,
            to_index,
            card,
            team_type: boost.team_type,
        })
    }
}

#[cfg(test)]
mod tests;
