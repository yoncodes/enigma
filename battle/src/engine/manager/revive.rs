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
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;
    use crate::engine::skill::rule::{DefinitionKey, RuleDomain};

    fn command() -> ReviveCommand {
        ReviveCommand {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(1043, "Revive"),
            },
            source_uid: 10,
            target_uid: 10,
            amount: 300,
            buff_uid: 20,
            dispel_statuses: Vec::new(),
        }
    }

    #[test]
    fn only_the_successful_revive_consumes_its_exact_buff() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(0),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31250181),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);

        let first = execute(&mut managers, command()).unwrap();
        let second = execute(&mut managers, command()).unwrap();

        assert_eq!(managers.hp.current(10), 300);
        assert!(first.hp.is_some());
        assert_eq!(first.buffs.len(), 1);
        assert_eq!(second, ReviveChanges::default());
    }
}
