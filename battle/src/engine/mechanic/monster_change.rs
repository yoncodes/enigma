use crate::engine::{
    manager::{
        BattleManagers,
        card::{CardChanges, CardCommand, CardCommandError, CardInvalidateAiOwner},
        entity::{EntityChanges, EntityCommand, EntityCommandError, EntityOperation},
        toughness::{ToughnessReset, ToughnessState},
    },
    skill::rule::CommandOrigin,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonsterChangeCommand {
    pub entity: EntityCommand,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MonsterChangeChanges {
    pub entity: EntityChanges,
    pub toughness: Option<ToughnessReset>,
    pub cards: Option<CardChanges>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonsterChangeError {
    InvalidCommand,
    Entity(EntityCommandError),
    Card(CardCommandError),
}

impl From<EntityCommandError> for MonsterChangeError {
    fn from(value: EntityCommandError) -> Self {
        Self::Entity(value)
    }
}

impl From<CardCommandError> for MonsterChangeError {
    fn from(value: CardCommandError) -> Self {
        Self::Card(value)
    }
}

pub(crate) fn execute(
    managers: &mut BattleManagers,
    command: MonsterChangeCommand,
) -> Result<MonsterChangeChanges, MonsterChangeError> {
    let EntityOperation::Transform { .. } = command.entity.operation else {
        return Err(MonsterChangeError::InvalidCommand);
    };
    let reset = command.entity.operation.resets_state();
    let target_uid = command.entity.target_uid;
    let team_type = managers
        .entity
        .team_type(target_uid)
        .ok_or(MonsterChangeError::InvalidCommand)?;
    let toughness_before: Option<ToughnessState> = managers.toughness.get(target_uid);
    let entity = managers.execute_entity(command.entity)?;
    if !reset {
        return Ok(MonsterChangeChanges {
            entity,
            toughness: None,
            cards: None,
        });
    }

    let toughness = Some(
        managers
            .toughness
            .settle_transform_reset(target_uid, toughness_before),
    );
    let cards = managers.execute_card(CardCommand::InvalidateAiOwner(CardInvalidateAiOwner {
        origin: CommandOrigin {
            domain: entity.origin.domain,
            key: entity.origin.key,
        },
        owner_uid: target_uid,
        team_type,
    }))?;
    let cards = (!cards.ai_invalidations.is_empty()).then_some(cards);

    Ok(MonsterChangeChanges {
        entity,
        toughness,
        cards,
    })
}

#[cfg(test)]
#[path = "monster_change/tests.rs"]
mod tests;
