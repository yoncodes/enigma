use sonettobuf::BuffActInfo;

use crate::engine::{
    damage::scale_permille,
    manager::{
        BattleManagers,
        buff::{BuffActInfoMarkerResult, BuffCommand, BuffSetState},
    },
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
    let mut buff = managers
        .buff
        .snapshot(owner_uid, feature.buff_uid)
        .ok_or(CounterError::InvalidState)?;
    let mut matching = buff
        .act_info
        .iter_mut()
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
    let next = current.saturating_add(amount).min(*maximum);
    if next == *current {
        return Ok(None);
    }
    let maximum = *maximum;
    info.param = vec![next, maximum];
    info.str_param = Some(String::new());
    Ok(Some(CounterUpdate {
        command: BuffCommand::SetInternalState(BuffSetState {
            origin: super::feature_command_origin(&feature).ok_or(CounterError::InvalidState)?,
            target_uid: owner_uid,
            buff_uid: feature.buff_uid,
            ex_info: None,
            params: None,
            act_info: Some(buff.act_info),
        }),
        marker: BuffActInfoMarkerResult {
            target_uid: owner_uid,
            buff_uid: feature.buff_uid,
            act_id,
            params: vec![next, maximum],
            str_param: Some(String::new()),
            team_type: 0,
        },
    }))
}

#[cfg(test)]
mod tests;
