use serde::{Deserialize, Serialize};
use sonettobuf::{CardData, CardExtraInfo, CardInfo, card_data::CardDataKey, card_info::CardType};

use super::{CardChange, CardCommandError, CardManager, command::CardInsertSovereign, permissions};

mod boost;
mod unlock;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Strengthen {
    #[serde(rename = "1")]
    first: u8,
    #[serde(rename = "2")]
    second: u8,
    #[serde(rename = "3")]
    third: u8,
    #[serde(rename = "4")]
    fourth: u8,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SovereignState {
    lock: bool,
    strengthen: Strengthen,
}

fn payload(card: &CardInfo) -> impl Iterator<Item = &CardData> {
    card.card_dataes
        .iter()
        .filter(|data| data.key == Some(CardDataKey::Unnamed as i32))
}

pub(super) fn is_locked(card: &CardInfo) -> Result<bool, ()> {
    let mut entries = payload(card);
    let Some(data) = entries.next() else {
        return Ok(false);
    };
    if entries.next().is_some() {
        return Err(());
    }
    let state: SovereignState =
        serde_json::from_str(data.value.as_deref().ok_or(())?).map_err(|_| ())?;
    let levels = state.strengthen;
    if [levels.first, levels.second, levels.third, levels.fourth]
        .iter()
        .any(|level| *level > 4)
    {
        return Err(());
    }
    Ok(state.lock)
}

impl CardManager {
    pub(crate) fn is_sovereign(card: &CardInfo) -> bool {
        payload(card).next().is_some()
    }

    pub(crate) fn has_sovereign_card(&self, owner_uid: i64) -> bool {
        self.hand()
            .iter()
            .chain(self.team_cards())
            .any(|card| card.uid == Some(owner_uid) && Self::is_sovereign(card))
    }

    pub(super) fn insert_sovereign(
        &mut self,
        insert: CardInsertSovereign,
    ) -> Result<Option<CardChange>, CardCommandError> {
        if insert.target_uid == 0
            || insert.skill_id <= 0
            || insert.hero_id <= 0
            || insert.max_position == 0
            || insert.max_position > i32::MAX as usize
            || insert.team_type != 1
        {
            return Err(CardCommandError::InvalidCommand);
        }
        if self.has_sovereign_card(insert.target_uid) {
            return Ok(None);
        }
        let value = serde_json::to_string(&SovereignState {
            lock: true,
            strengthen: Strengthen::default(),
        })
        .map_err(|_| CardCommandError::InvalidCommand)?;
        let card = CardInfo {
            uid: Some(insert.target_uid),
            skill_id: Some(insert.skill_id),
            hero_id: Some(insert.hero_id),
            card_type: Some(CardType::Skill3 as i32),
            temp_card: Some(false),
            card_dataes: vec![CardData {
                key: Some(CardDataKey::Unnamed as i32),
                value: Some(value),
            }],
            extra_infos: [
                permissions::NOT_COMPOSE,
                permissions::NOT_MOVE,
                permissions::NOT_USE,
            ]
            .into_iter()
            .map(|key| CardExtraInfo {
                key: Some(key),
                values: vec!["UnnamedSpCard".to_owned()],
            })
            .collect(),
            ..super::temp_card(insert.skill_id)
        };
        let index = self.hand().len().min(insert.max_position - 1);
        self.deck
            .insert_hand(index, card.clone())
            .ok_or(CardCommandError::InvalidCommand)?;
        Ok(Some(CardChange::InsertHand {
            target_uid: insert.target_uid,
            card,
            index,
            team_type: insert.team_type,
        }))
    }
}

#[cfg(test)]
mod tests;
