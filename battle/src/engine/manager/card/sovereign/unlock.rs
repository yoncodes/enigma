use super::{CardChange, CardCommandError, CardManager, is_locked, permissions, state};
use sonettobuf::card_data::CardDataKey;

impl CardManager {
    pub(crate) fn can_unlock_sovereign(&self, owner_uid: i64) -> Result<bool, CardCommandError> {
        let Some(card) = self
            .hand()
            .first()
            .filter(|card| card.uid == Some(owner_uid))
        else {
            return Ok(false);
        };
        is_locked(card).map_err(|_| CardCommandError::InvalidCommand)
    }

    pub(in crate::engine::manager::card) fn unlock_sovereign(
        &mut self,
        owner_uid: i64,
        team_type: i32,
    ) -> Result<CardChange, CardCommandError> {
        if owner_uid == 0 || team_type != 1 || !self.can_unlock_sovereign(owner_uid)? {
            return Err(CardCommandError::InvalidCommand);
        }
        let mut card = self.hand()[0].clone();
        let mut state = state(&card)
            .map_err(|_| CardCommandError::InvalidCommand)?
            .ok_or(CardCommandError::InvalidCommand)?;
        let data = card
            .card_dataes
            .iter_mut()
            .find(|data| data.key == Some(CardDataKey::Unnamed as i32))
            .ok_or(CardCommandError::InvalidCommand)?;
        state.lock = false;
        data.value =
            Some(serde_json::to_string(&state).map_err(|_| CardCommandError::InvalidCommand)?);
        card.extra_infos.retain_mut(|info| {
            if info.key != Some(permissions::NOT_USE) {
                return true;
            }
            info.values.retain(|reason| reason != "UnnamedSpCard");
            !info.values.is_empty()
        });
        self.deck.hand_mut()[0] = card.clone();
        Ok(CardChange::SovereignUnlocked { card, team_type })
    }
}
