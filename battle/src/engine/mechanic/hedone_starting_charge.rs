use sonettobuf::BuffActInfo;

use crate::engine::{
    manager::{
        BattleManagers,
        buff::{BuffActInfoMarkerResult, BuffChanges, BuffCommand, BuffCommandError, BuffSetState},
        card::{CardChanges, CardCommand, CardCommandError, CardUnlockHedone},
    },
    skill::{
        buff_act::hedone_card::{HedoneCardError, owner_state},
        rule::CommandOrigin,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HedoneStartingChargeCommand {
    pub origin: CommandOrigin,
    pub owner_uid: i64,
    pub amount: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HedoneStartingChargeChanges {
    pub card: CardChanges,
    pub buff: BuffChanges,
    pub marker: BuffActInfoMarkerResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HedoneStartingChargeError {
    InvalidCommand,
    State(HedoneCardError),
    Card(CardCommandError),
    Buff(BuffCommandError),
}

impl From<HedoneCardError> for HedoneStartingChargeError {
    fn from(value: HedoneCardError) -> Self {
        Self::State(value)
    }
}

impl From<CardCommandError> for HedoneStartingChargeError {
    fn from(value: CardCommandError) -> Self {
        Self::Card(value)
    }
}

impl From<BuffCommandError> for HedoneStartingChargeError {
    fn from(value: BuffCommandError) -> Self {
        Self::Buff(value)
    }
}

pub(crate) fn execute(
    managers: &mut BattleManagers,
    command: HedoneStartingChargeCommand,
) -> Result<HedoneStartingChargeChanges, HedoneStartingChargeError> {
    let state = owner_state(managers, command.owner_uid)?
        .filter(|state| {
            !state.unlocked
                && command.amount >= state.config.unlock_threshold
                && command.amount <= state.maximum
        })
        .ok_or(HedoneStartingChargeError::InvalidCommand)?;
    let params = vec![command.amount, state.maximum, 1];
    let buff_plan = managers.plan_buff(BuffCommand::SetInternalState(BuffSetState {
        origin: command.origin,
        target_uid: command.owner_uid,
        buff_uid: state.buff_uid,
        ex_info: None,
        params: None,
        act_info: Some(vec![BuffActInfo {
            act_id: Some(state.act_id),
            param: params.clone(),
            str_param: Some(String::new()),
        }]),
    }))?;
    let card = managers.execute_card(CardCommand::UnlockHedone(CardUnlockHedone {
        origin: command.origin,
        owner_uid: command.owner_uid,
        skill_id: state.config.skill_id,
        team_type: state.team_type,
    }))?;
    let buff = managers.commit_buff(buff_plan);
    Ok(HedoneStartingChargeChanges {
        card,
        buff,
        marker: BuffActInfoMarkerResult {
            target_uid: command.owner_uid,
            buff_uid: state.buff_uid,
            act_id: state.act_id,
            params,
            str_param: Some(String::new()),
            team_type: 0,
        },
    })
}

#[cfg(test)]
mod tests;
