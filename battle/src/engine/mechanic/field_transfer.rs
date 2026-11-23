use crate::engine::{
    manager::{
        BattleManagers,
        buff::{
            BuffAccumulateActValue, BuffChanges, BuffCommand, BuffCommandError, BuffConsume,
            BuffSelector, DepletedBuff,
        },
        field::{FieldChange, FieldCommand, FieldCommandError, FieldOperation},
    },
    skill::rule::CommandOrigin,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldTransferCommand {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub buff: BuffSelector,
    pub limit: i32,
    pub team: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldTransferChanges {
    pub buffs: Vec<BuffChanges>,
    pub field: FieldChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldTransferError {
    InvalidCommand,
    MissingBuff,
    MissingProgressField(i32),
    Buff(BuffCommandError),
    Field(FieldCommandError),
}

impl From<BuffCommandError> for FieldTransferError {
    fn from(value: BuffCommandError) -> Self {
        Self::Buff(value)
    }
}

impl From<FieldCommandError> for FieldTransferError {
    fn from(value: FieldCommandError) -> Self {
        Self::Field(value)
    }
}

pub(crate) fn execute(
    managers: &mut BattleManagers,
    command: FieldTransferCommand,
) -> Result<FieldTransferChanges, FieldTransferError> {
    if command.target_uid == 0 || command.team == 0 || command.limit <= 0 {
        return Err(FieldTransferError::InvalidCommand);
    }
    if managers
        .field
        .get(command.team)
        .is_none_or(|field| field.next_upgrade_progress <= 0)
    {
        return Err(FieldTransferError::MissingProgressField(command.team));
    }
    let BuffSelector::IdOrType(selector) = command.buff else {
        return Err(FieldTransferError::InvalidCommand);
    };
    let amount = managers
        .buff
        .buff_id_or_type_amount(command.target_uid, selector)
        .min(command.limit);
    if amount <= 0 {
        return Err(FieldTransferError::MissingBuff);
    }

    let extras = crate::engine::skill::buff_act::electric_transform::extra_transfers(
        &managers.buff,
        &managers.hp,
        command.target_uid,
    );
    let extra_amount = extras
        .iter()
        .map(|extra| extra.progress)
        .fold(0_i32, i32::saturating_add);
    let records = crate::engine::skill::buff_act::electric_transform::transfer_records(
        &managers.buff,
        &managers.hp,
        command.team,
        command.target_uid,
        extra_amount,
    );
    let mut next_buffs = managers.buff.clone();
    let mut buff_changes = Vec::with_capacity(1 + extras.len());
    let buff = next_buffs.plan_with_source_attack(
        &managers.hp,
        BuffCommand::ConsumeCoalesced(BuffConsume {
            origin: command.origin,
            target_uid: command.target_uid,
            selector: command.buff,
            amount,
            depleted: DepletedBuff::Remove,
        }),
        crate::engine::manager::buff::GrantInputs::default(),
    )?;
    buff_changes.push(next_buffs.commit(&managers.hp, buff));
    for extra in extras {
        let buff = next_buffs.plan_with_source_attack(
            &managers.hp,
            BuffCommand::ConsumeCoalesced(BuffConsume {
                origin: command.origin,
                target_uid: command.target_uid,
                selector: BuffSelector::Uid(extra.buff_uid),
                amount: extra.consumed_layers,
                depleted: DepletedBuff::Remove,
            }),
            crate::engine::manager::buff::GrantInputs::default(),
        )?;
        buff_changes.push(next_buffs.commit(&managers.hp, buff));
    }
    for record in records {
        let update = next_buffs.plan_with_source_attack(
            &managers.hp,
            BuffCommand::AccumulateActValue(BuffAccumulateActValue {
                origin: record.origin,
                target_uid: record.owner_uid,
                buff_uid: record.buff_uid,
                act_id: record.act_id,
                delta: record.delta,
            }),
            crate::engine::manager::buff::GrantInputs::default(),
        )?;
        next_buffs.commit(&managers.hp, update);
    }
    let field = managers.field.plan_command(FieldCommand {
        origin: command.origin,
        team: command.team,
        operation: FieldOperation::ChangeProgress {
            delta: amount.saturating_add(extra_amount),
        },
    })?;

    managers.buff = next_buffs;
    let field = managers.field.commit(field);
    managers.field.record_transfer(command.team);
    Ok(FieldTransferChanges {
        buffs: buff_changes,
        field,
    })
}

#[cfg(test)]
mod tests;
