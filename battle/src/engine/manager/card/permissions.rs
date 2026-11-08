use sonettobuf::CardInfo;

use super::{
    CardChange, CardCommandError, CardManager, CardPlayChoice, command::CardUnlockHedone, sovereign,
};

pub(super) const NOT_COMPOSE: i32 = 1;
pub(super) const NOT_MOVE: i32 = 4;
pub(super) const NOT_USE: i32 = 5;
pub(super) const NOT_USE_ACTION_POINT: i32 = 8;

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

    pub(super) fn unlock_hedone(
        &mut self,
        command: CardUnlockHedone,
    ) -> Result<CardChange, CardCommandError> {
        if command.owner_uid == 0 || command.skill_id <= 0 || command.team_type != 1 {
            return Err(CardCommandError::InvalidCommand);
        }
        let mut matches = self.hand().iter().enumerate().filter(|(_, card)| {
            card.uid == Some(command.owner_uid)
                && card.skill_id == Some(command.skill_id)
                && super::hedone_card::tagged(card, NOT_USE_ACTION_POINT)
        });
        let Some((index, card)) = matches.next() else {
            return Err(CardCommandError::InvalidCommand);
        };
        if matches.next().is_some() || !super::hedone_card::tagged(card, NOT_USE) {
            return Err(CardCommandError::InvalidCommand);
        }
        let mut card = card.clone();
        card.extra_infos.retain_mut(|info| {
            if info.key != Some(NOT_USE) {
                return true;
            }
            info.values.retain(|value| value != super::hedone_card::TAG);
            !info.values.is_empty()
        });
        self.deck.hand_mut()[index] = card.clone();
        Ok(CardChange::HedoneUnlocked {
            card,
            index,
            team_type: command.team_type,
        })
    }
}
