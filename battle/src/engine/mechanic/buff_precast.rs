use crate::engine::{
    manager::{
        BattleManagers,
        buff::{
            BuffChanges, BuffCommand, BuffCommandError, BuffConsume, BuffSelector, DepletedBuff,
        },
        card::{CardAddPrecast, CardChanges, CardCommand, CardCommandError, selected_precast_card},
    },
    skill::rule::CommandOrigin,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffPrecastOption {
    pub cost: i32,
    pub skill_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuffPrecastCommand {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub buff_id: i32,
    pub options: Vec<BuffPrecastOption>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuffPrecastChanges {
    pub buff: BuffChanges,
    pub card: CardChanges,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffPrecastError {
    InvalidCommand,
    Buff(BuffCommandError),
    Card(CardCommandError),
}

impl From<BuffCommandError> for BuffPrecastError {
    fn from(value: BuffCommandError) -> Self {
        Self::Buff(value)
    }
}

impl From<CardCommandError> for BuffPrecastError {
    fn from(value: CardCommandError) -> Self {
        Self::Card(value)
    }
}

pub(crate) fn execute(
    managers: &mut BattleManagers,
    command: BuffPrecastCommand,
) -> Result<Option<BuffPrecastChanges>, BuffPrecastError> {
    if command.source_uid == 0
        || command.target_uid == 0
        || command.buff_id <= 0
        || command.options.is_empty()
        || command
            .options
            .iter()
            .any(|option| option.cost <= 0 || option.skill_id <= 0)
    {
        return Err(BuffPrecastError::InvalidCommand);
    }

    let amount = managers
        .buff
        .buff_id_amount(command.target_uid, command.buff_id);
    let Some(option) = command.options.iter().find(|option| option.cost <= amount) else {
        return Ok(None);
    };
    let hero_id = managers
        .entity
        .model_id(command.source_uid)
        .ok_or(BuffPrecastError::InvalidCommand)?;

    let buff = managers.execute_buff(BuffCommand::Consume(BuffConsume {
        origin: command.origin,
        target_uid: command.target_uid,
        selector: BuffSelector::ExactId(command.buff_id),
        amount: option.cost,
        depleted: DepletedBuff::Remove,
    }))?;
    let card = managers.execute_card(CardCommand::AddSelectedPrecast(CardAddPrecast {
        origin: command.origin,
        card: selected_precast_card(command.source_uid, hero_id, option.skill_id),
    }))?;

    Ok(Some(BuffPrecastChanges { buff, card }))
}
