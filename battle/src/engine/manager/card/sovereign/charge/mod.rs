use super::{CardChange, CardCommandError, CardManager, SovereignState, state};
use sonettobuf::{CardInfo, card_data::CardDataKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SovereignEnhancement {
    Single,
    Mass,
    Support,
    Recovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChargeTarget {
    Hand,
    Queued(i32),
}

fn required_state(card: &CardInfo) -> Result<SovereignState, CardCommandError> {
    state(card)
        .map_err(|_| CardCommandError::InvalidCommand)?
        .ok_or(CardCommandError::InvalidCommand)
}

impl SovereignState {
    fn level(&self, kind: SovereignEnhancement) -> u8 {
        match kind {
            SovereignEnhancement::Single => self.strengthen.first,
            SovereignEnhancement::Mass => self.strengthen.second,
            SovereignEnhancement::Support => self.strengthen.third,
            SovereignEnhancement::Recovery => self.strengthen.fourth,
        }
    }

    fn level_mut(&mut self, kind: SovereignEnhancement) -> &mut u8 {
        match kind {
            SovereignEnhancement::Single => &mut self.strengthen.first,
            SovereignEnhancement::Mass => &mut self.strengthen.second,
            SovereignEnhancement::Support => &mut self.strengthen.third,
            SovereignEnhancement::Recovery => &mut self.strengthen.fourth,
        }
    }
}

impl CardManager {
    pub(crate) fn charge_target(
        &self,
        owner_uid: i64,
        kind: SovereignEnhancement,
        after_index: i32,
    ) -> Result<Option<ChargeTarget>, CardCommandError> {
        if let Some(card) = self
            .hand()
            .iter()
            .find(|card| card.uid == Some(owner_uid) && Self::is_sovereign(card))
        {
            return Ok((required_state(card)?.level(kind) < 4).then_some(ChargeTarget::Hand));
        }
        if let Some(played) = self.played().iter().find(|played| {
            played.card.uid == Some(owner_uid)
                && Self::is_sovereign(&played.card)
                && played.card_index > after_index
        }) {
            return Ok((required_state(&played.card)?.level(kind) < 4)
                .then_some(ChargeTarget::Queued(played.card_index)));
        }
        Ok(None)
    }

    pub(in crate::engine::manager::card) fn charge_sovereign(
        &mut self,
        owner_uid: i64,
        kind: SovereignEnhancement,
        team_type: i32,
    ) -> Result<CardChange, CardCommandError> {
        if owner_uid == 0 || team_type != 1 {
            return Err(CardCommandError::InvalidCommand);
        }
        let (index, card) = self
            .hand()
            .iter()
            .enumerate()
            .find(|(_, card)| card.uid == Some(owner_uid) && Self::is_sovereign(card))
            .ok_or(CardCommandError::InvalidCommand)?;
        let mut state = required_state(card)?;
        let level = state.level_mut(kind);
        if *level >= 4 {
            return Err(CardCommandError::InvalidCommand);
        }
        *level += 1;
        let mut card = card.clone();
        card.card_dataes
            .iter_mut()
            .find(|data| data.key == Some(CardDataKey::Unnamed as i32))
            .ok_or(CardCommandError::InvalidCommand)?
            .value =
            Some(serde_json::to_string(&state).map_err(|_| CardCommandError::InvalidCommand)?);
        self.deck.hand_mut()[index] = card.clone();
        Ok(CardChange::SovereignCharged {
            card,
            index,
            team_type,
        })
    }
}
