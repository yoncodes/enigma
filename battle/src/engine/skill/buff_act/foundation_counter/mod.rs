use sonettobuf::{BuffActInfo, BuffInfo};

use crate::engine::{
    damage::scale_permille,
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffActInfoMarkerResult, BuffCommand, BuffSetState},
    },
    skill::rule::CommandOrigin,
};

use super::registry::BuffActKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterError {
    InvalidState,
    MissingCarrier,
    MultipleCarriers,
}

pub struct CounterUpdate {
    pub command: BuffCommand,
    pub marker: BuffActInfoMarkerResult,
}

struct CounterState {
    feature: ActiveBuffFeature,
    buff: BuffInfo,
    act_id: i32,
    current: i32,
    maximum: i32,
}

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [capacity, first_rate, second_rate]
        if *capacity > 0 && *first_rate > 0 && *second_rate > 0)
}

pub fn initial_act_info(source_attack: i32, values: &[i32]) -> Option<BuffActInfo> {
    let [act_id, capacity, first_rate, second_rate] = values else {
        return None;
    };
    supports(&[*capacity, *first_rate, *second_rate]).then(|| BuffActInfo {
        act_id: Some(*act_id),
        param: vec![0, scale_permille(source_attack, *capacity)],
        str_param: Some(String::new()),
    })
}

pub fn update(
    managers: &BattleManagers,
    owner_uid: i64,
    amount: i32,
) -> Result<Option<CounterUpdate>, CounterError> {
    if amount <= 0 {
        return Ok(None);
    }
    let state = state(managers, owner_uid)?;
    let next = state.current.saturating_add(amount).min(state.maximum);
    if next == state.current {
        return Ok(None);
    }
    let origin = super::feature_command_origin(&state.feature).ok_or(CounterError::InvalidState)?;
    Ok(Some(set(state, next, origin)?))
}

pub fn consume(
    managers: &BattleManagers,
    owner_uid: i64,
    origin: CommandOrigin,
) -> Result<Option<(CounterUpdate, i32)>, CounterError> {
    let state = state(managers, owner_uid)?;
    if state.current <= 0 {
        return Ok(None);
    }
    let current = state.current;
    Ok(Some((set(state, 0, origin)?, current)))
}

fn state(managers: &BattleManagers, owner_uid: i64) -> Result<CounterState, CounterError> {
    let mut features = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| {
            feature.owner_uid == owner_uid
                && super::is_kind(feature, BuffActKind::FoundationCounter)
        });
    let Some(feature) = features.next() else {
        return Err(CounterError::MissingCarrier);
    };
    if features.next().is_some() {
        return Err(CounterError::MultipleCarriers);
    }
    let act_id = feature.act_id().ok_or(CounterError::InvalidState)?;
    let buff = managers
        .buff
        .snapshot(owner_uid, feature.buff_uid)
        .ok_or(CounterError::InvalidState)?;
    let mut matching = buff
        .act_info
        .iter()
        .filter(|info| info.act_id == Some(act_id));
    let info = matching.next().ok_or(CounterError::InvalidState)?;
    if matching.next().is_some() || info.str_param.as_deref() != Some("") {
        return Err(CounterError::InvalidState);
    }
    let [current, maximum] = info.param.as_slice() else {
        return Err(CounterError::InvalidState);
    };
    if *maximum <= 0 || !(0..=*maximum).contains(current) {
        return Err(CounterError::InvalidState);
    }
    let current = *current;
    let maximum = *maximum;
    Ok(CounterState {
        feature,
        buff,
        act_id,
        current,
        maximum,
    })
}

fn set(
    mut state: CounterState,
    next: i32,
    origin: CommandOrigin,
) -> Result<CounterUpdate, CounterError> {
    let info = state
        .buff
        .act_info
        .iter_mut()
        .find(|info| info.act_id == Some(state.act_id))
        .ok_or(CounterError::InvalidState)?;
    let maximum = state.maximum;
    info.param = vec![next, maximum];
    info.str_param = Some(String::new());
    Ok(CounterUpdate {
        command: BuffCommand::SetInternalState(BuffSetState {
            origin,
            target_uid: state.feature.owner_uid,
            buff_uid: state.feature.buff_uid,
            ex_info: None,
            params: None,
            act_info: Some(state.buff.act_info),
        }),
        marker: BuffActInfoMarkerResult {
            target_uid: state.feature.owner_uid,
            buff_uid: state.feature.buff_uid,
            act_id: state.act_id,
            params: vec![next, maximum],
            str_param: Some(String::new()),
            team_type: 0,
        },
    })
}

#[cfg(test)]
mod tests;
