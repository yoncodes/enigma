use sonettobuf::{CardExtraInfo, CardInfo};

use super::{CardChange, CardCommandError, CardManager, command::CardAddHedone};

pub(super) const TAG: &str = "HedoneCard";

pub(super) fn tagged(card: &CardInfo, key: i32) -> bool {
    card.extra_infos
        .iter()
        .any(|info| info.key == Some(key) && info.values.iter().any(|value| value == TAG))
}

impl CardManager {
    pub(crate) fn has_hedone_card(&self, owner_uid: i64, skill_id: i32) -> bool {
        self.hand().iter().any(|card| {
            card.uid == Some(owner_uid)
                && card.skill_id == Some(skill_id)
                && tagged(card, super::permissions::NOT_USE_ACTION_POINT)
        })
    }

    pub(super) fn add_hedone(
        &mut self,
        command: CardAddHedone,
    ) -> Result<CardChange, CardCommandError> {
        if command.owner_uid == 0
            || command.skill_id <= 0
            || command.hero_id <= 0
            || command.team_type != 1
            || self.has_hedone_card(command.owner_uid, command.skill_id)
        {
            return Err(CardCommandError::InvalidCommand);
        }
        let index = self.hand().len();
        let card = self.add_to_hand_for(
            command.owner_uid,
            CardInfo {
                uid: Some(command.owner_uid),
                skill_id: Some(command.skill_id),
                hero_id: Some(command.hero_id),
                temp_card: Some(false),
                extra_infos: [
                    super::permissions::NOT_USE,
                    super::permissions::NOT_USE_ACTION_POINT,
                ]
                .into_iter()
                .map(|key| CardExtraInfo {
                    key: Some(key),
                    values: vec![TAG.to_owned()],
                })
                .collect(),
                ..Default::default()
            },
        );
        Ok(CardChange::HedoneAdd {
            card,
            index,
            team_type: command.team_type,
        })
    }
}
