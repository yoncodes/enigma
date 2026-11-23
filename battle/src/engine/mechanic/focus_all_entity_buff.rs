use crate::engine::{
    manager::{
        BattleManagers,
        buff::{
            BuffChanges, BuffChildUidReservation, BuffCommand, BuffCommandError, BuffGrant,
            BuffRemove, BuffRemoveSelector,
        },
    },
    skill::rule::{CommandOrigin, output::EffectMarker},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusAllEntityBuffCommand {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub buff_id: i32,
    pub candidate_uids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FocusAllEntityBuffChanges {
    pub buffs: Vec<BuffChanges>,
    pub marker: EffectMarker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusAllEntityBuffError {
    InvalidCommand,
    UnsupportedDonorAmount { owner_uid: i64, amount: i32 },
    Buff(BuffCommandError),
}

impl From<BuffCommandError> for FocusAllEntityBuffError {
    fn from(value: BuffCommandError) -> Self {
        Self::Buff(value)
    }
}

pub(crate) fn execute(
    managers: &mut BattleManagers,
    command: FocusAllEntityBuffCommand,
) -> Result<Option<FocusAllEntityBuffChanges>, FocusAllEntityBuffError> {
    if command.source_uid == 0
        || command.target_uid == 0
        || command.buff_id <= 0
        || command.candidate_uids.is_empty()
    {
        return Err(FocusAllEntityBuffError::InvalidCommand);
    }
    let capacity = managers.buff.stack_limit(command.buff_id);
    let current = managers
        .buff
        .buff_id_amount(command.target_uid, command.buff_id);
    if capacity <= 0 || current <= 0 || current >= capacity {
        return Ok(None);
    }

    let mut candidates = Vec::new();
    for owner_uid in command
        .candidate_uids
        .iter()
        .copied()
        .filter(|uid| *uid != command.source_uid)
    {
        let Some(buff_uid) = managers.buff.buff_id_uid(owner_uid, command.buff_id) else {
            continue;
        };
        let amount = managers.buff.buff_id_amount(owner_uid, command.buff_id);
        if amount != 1 {
            return Err(FocusAllEntityBuffError::UnsupportedDonorAmount { owner_uid, amount });
        }
        candidates.push((owner_uid, buff_uid));
        if candidates.len() == (capacity - current) as usize {
            break;
        }
    }
    if candidates.is_empty() {
        return Ok(None);
    }

    let mut next = managers.clone();
    next.buff.begin_transaction();
    let transaction = (|| {
        let mut changes = Vec::with_capacity(candidates.len() * 2);
        let mut reservation_uids = Vec::with_capacity(candidates.len());
        for (owner_uid, buff_uid) in &candidates {
            changes.push(next.execute_buff(BuffCommand::Remove(BuffRemove {
                origin: command.origin,
                target_uid: *owner_uid,
                selector: BuffRemoveSelector::Uid(*buff_uid),
            }))?);
            let reservation =
                next.plan_buff(BuffCommand::ReserveChildUids(BuffChildUidReservation {
                    origin: command.origin,
                    target_uid: command.target_uid,
                    count: 1,
                }))?;
            let planned_uids = reservation
                .planned_reservation_uids()
                .ok_or(FocusAllEntityBuffError::InvalidCommand)?;
            let [reservation_uid] = planned_uids.as_slice() else {
                return Err(FocusAllEntityBuffError::InvalidCommand);
            };
            reservation_uids.push(*reservation_uid);
            next.commit_buff(reservation);
            let plan = next.plan_buff(BuffCommand::Accumulate(BuffGrant {
                origin: command.origin,
                source_uid: command.source_uid,
                target_uid: command.target_uid,
                buff_id: command.buff_id,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }))?;
            changes.push(next.commit_buff(plan));
        }
        Ok::<_, FocusAllEntityBuffError>((changes, reservation_uids))
    })();
    next.buff.end_transaction();
    let (buffs, reservation_uids) = transaction?;
    *managers = next;

    let reservations = reservation_uids
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join("#");
    let donors = candidates
        .iter()
        .map(|(owner_uid, buff_uid)| format!("{owner_uid}:{buff_uid}"))
        .collect::<Vec<_>>()
        .join("|");
    Ok(Some(FocusAllEntityBuffChanges {
        buffs,
        marker: EffectMarker {
            target_uid: command.target_uid,
            effect_type: sonettobuf::effect_type_enum::EffectType::Ananfocusbuff as i32,
            effect_num: 0,
            effect_num1: None,
            config_effect: command.origin.key.opcode,
            reserve_id: Some(0),
            reserve_str: Some(format!("{reservations},{donors}")),
        },
    }))
}

#[cfg(test)]
mod tests;
