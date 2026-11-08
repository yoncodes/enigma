use crate::engine::{
    manager::{
        BattleManagers,
        conduit::{
            ConduitChange, ConduitCommand, ConduitError, ConduitPowerChange, ConduitPowerChangeKind,
        },
    },
    skill::{
        buff_act::hedone_card::{HedoneCardError, owner_state},
        rule::CommandOrigin,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HedoneDevicePowerCommand {
    pub origin: CommandOrigin,
    pub owner_uid: i64,
    pub power_id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HedoneDevicePowerError {
    InvalidCommand,
    State(HedoneCardError),
    Conduit(ConduitError),
}

impl From<HedoneCardError> for HedoneDevicePowerError {
    fn from(value: HedoneCardError) -> Self {
        Self::State(value)
    }
}

impl From<ConduitError> for HedoneDevicePowerError {
    fn from(value: ConduitError) -> Self {
        Self::Conduit(value)
    }
}

pub(crate) fn execute(
    managers: &mut BattleManagers,
    command: HedoneDevicePowerCommand,
) -> Result<ConduitChange, HedoneDevicePowerError> {
    if command.owner_uid == 0 || command.power_id <= 0 {
        return Err(HedoneDevicePowerError::InvalidCommand);
    }
    let state = owner_state(managers, command.owner_uid)?
        .filter(|state| state.unlocked && state.stored > 0)
        .ok_or(HedoneDevicePowerError::InvalidCommand)?;
    let delta = state
        .stored
        .checked_mul(state.config.power_add)
        .ok_or(HedoneDevicePowerError::InvalidCommand)?;
    Ok(managers
        .conduit
        .execute(ConduitCommand::ChangePower(ConduitPowerChange {
            origin: command.origin,
            source_uid: state.owner_uid,
            team: state.team_type,
            power_id: command.power_id,
            delta,
            kind: ConduitPowerChangeKind::Standard,
        }))?)
}
