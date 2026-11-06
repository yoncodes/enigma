use sonettobuf::CardInfo;

use super::{CardCommandError, CardManager, CardPlayChoice, sovereign};

pub(super) const NOT_COMPOSE: i32 = 1;
pub(super) const NOT_MOVE: i32 = 4;
pub(super) const NOT_USE: i32 = 5;

pub(super) fn permits(card: &CardInfo, restriction: i32) -> bool {
    !card
        .extra_infos
        .iter()
        .any(|info| info.key == Some(restriction))
}

impl CardManager {
    pub(crate) fn can_use_card(&self, card: &CardInfo) -> bool {
        permits(card, NOT_USE) && sovereign::is_locked(card).is_ok_and(|locked| !locked)
    }

    pub(crate) fn check_play_permission(
        &self,
        index: usize,
        choice: Option<&CardPlayChoice>,
    ) -> Result<(), CardCommandError> {
        let source = self
            .resolve_play_source(index, choice)
            .ok_or(if choice.is_none() {
                CardCommandError::InvalidPlayIndex
            } else {
                CardCommandError::PlayedCardNotFound
            })?;
        if !self.can_use_card(self.play_source_card(source)) {
            return Err(CardCommandError::PlayForbidden);
        }
        Ok(())
    }
}
