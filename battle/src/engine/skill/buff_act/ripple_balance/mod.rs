use crate::engine::{
    damage::scale_permille,
    manager::{
        BattleManagers,
        buff::{
            BuffActInfoMarkerResult, BuffCommand, BuffRemove, BuffRemoveSelector, BuffSetState,
        },
    },
};

use super::registry::BuffActKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidBalance,
    MultipleBalances,
}

pub struct Mitigation {
    pub damage: i32,
    pub clear: Option<Clear>,
}

pub struct Clear {
    pub reset: BuffCommand,
    pub remove: BuffCommand,
    pub marker: BuffActInfoMarkerResult,
}

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [300 | 400])
}

pub fn mitigate(
    managers: &BattleManagers,
    target_uid: i64,
    damage: i32,
) -> Result<Option<Mitigation>, Error> {
    if damage <= 0
        || !managers
            .buff
            .has_buff_act_kind(target_uid, BuffActKind::RippleBalance)
    {
        return Ok(None);
    }
    let mut active = managers
        .buff
        .active_features_where(&managers.hp, |opcode, type_name, _| {
            opcode
                .and_then(|opcode| super::registry::find(opcode, type_name))
                .is_some_and(|definition| definition.kind == BuffActKind::RippleBalance)
        })
        .into_iter()
        .filter(|feature| feature.owner_uid == target_uid && feature.owner_alive);
    let Some(feature) = active.next() else {
        return Ok(None);
    };
    if active.next().is_some() {
        return Err(Error::MultipleBalances);
    }
    let [act_id, rate] = feature.values.as_slice() else {
        return Err(Error::InvalidBalance);
    };
    let mut buff = managers
        .buff
        .snapshot(target_uid, feature.buff_uid)
        .ok_or(Error::InvalidBalance)?;
    let info = buff
        .act_info
        .iter_mut()
        .find(|info| info.act_id == Some(*act_id))
        .ok_or(Error::InvalidBalance)?;
    let [balance] = info.param.as_mut_slice() else {
        return Err(Error::InvalidBalance);
    };
    if *balance < 0 {
        return Err(Error::InvalidBalance);
    }
    if *balance == 0 {
        return Ok(None);
    }
    if damage < *balance {
        return Ok(Some(Mitigation {
            damage: 0,
            clear: None,
        }));
    }

    *balance = 0;
    let origin = super::feature_command_origin(&feature).ok_or(Error::InvalidBalance)?;
    let buff_uid = feature.buff_uid;
    Ok(Some(Mitigation {
        damage: damage - scale_permille(damage, *rate),
        clear: Some(Clear {
            reset: BuffCommand::SetInternalState(BuffSetState {
                origin,
                target_uid,
                buff_uid,
                ex_info: None,
                params: None,
                act_info: Some(buff.act_info),
            }),
            remove: BuffCommand::RemoveAfterTrigger(BuffRemove {
                origin,
                target_uid,
                selector: BuffRemoveSelector::Uid(buff_uid),
            }),
            marker: BuffActInfoMarkerResult {
                target_uid,
                buff_uid,
                act_id: *act_id,
                params: vec![0],
                str_param: Some(String::new()),
                team_type: 0,
            },
        }),
    }))
}

#[cfg(test)]
mod tests;
