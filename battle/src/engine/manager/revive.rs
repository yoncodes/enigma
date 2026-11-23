use crate::engine::{
    manager::{
        BattleManagers,
        buff::{
            BuffChanges, BuffCommand, BuffCommandError, BuffConsume, BuffDispel, BuffSelector,
            BuffStatus, DepletedBuff,
        },
        hp::{HpChanges, HpCommand, HpCommandError, HpHeal, HpHealKind},
    },
    skill::rule::CommandOrigin,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviveCommand {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub amount: i32,
    pub buff_uid: i64,
    pub dispel_statuses: Vec<BuffStatus>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReviveChanges {
    pub hp: Option<Box<HpChanges>>,
    pub buffs: Vec<BuffChanges>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviveError {
    Hp(HpCommandError),
    Buff(BuffCommandError),
}

pub fn execute(
    managers: &mut BattleManagers,
    command: ReviveCommand,
) -> Result<ReviveChanges, ReviveError> {
    if command.amount <= 0 || managers.hp.current(command.target_uid) > 0 {
        return Ok(ReviveChanges::default());
    }
    let hp = managers
        .execute_hp(HpCommand::Heal(HpHeal {
            origin: command.origin,
            source_uid: command.source_uid,
            target_uid: command.target_uid,
            amount: command.amount,
            config_effect: command.origin.key.opcode,
            kind: HpHealKind::Revive,
        }))
        .map_err(ReviveError::Hp)?;
    if hp.hp.is_none_or(|change| change.delta <= 0) {
        return Ok(ReviveChanges::default());
    }
    let mut buffs = Vec::new();
    if !command.dispel_statuses.is_empty() {
        buffs.push(
            managers
                .execute_buff(BuffCommand::Dispel(BuffDispel {
                    origin: command.origin,
                    target_uid: command.target_uid,
                    statuses: command.dispel_statuses,
                    excluded_ids_or_types: Vec::new(),
                    count: 0,
                }))
                .map_err(ReviveError::Buff)?,
        );
    }
    buffs.push(
        managers
            .execute_buff(BuffCommand::Consume(BuffConsume {
                origin: command.origin,
                target_uid: command.target_uid,
                selector: BuffSelector::Uid(command.buff_uid),
                amount: 1,
                depleted: DepletedBuff::Remove,
            }))
            .map_err(ReviveError::Buff)?,
    );
    Ok(ReviveChanges {
        hp: Some(Box::new(hp)),
        buffs,
    })
}

#[cfg(test)]
mod tests;
