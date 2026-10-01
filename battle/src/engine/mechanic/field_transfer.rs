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
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    use super::*;
    use crate::engine::{
        entity::attr::AttrId,
        manager::{
            buff::BuffGrant,
            field::{FieldDefinition, FieldOperation, FieldThreshold},
        },
        skill::rule::{DefinitionKey, RuleDomain},
    };

    const ORIGIN: CommandOrigin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60195, "ElectricTransform"),
    };

    #[test]
    fn transfer_commits_buff_consumption_and_field_progress_together() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        buff_id: Some(90071),
                        uid: Some(2),
                        layer: Some(6),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        managers
            .execute_field(FieldCommand {
                origin: ORIGIN,
                team: 1,
                operation: FieldOperation::DeployIfAbsent {
                    definition: FieldDefinition {
                        field_id: 30001,
                        duration: 3,
                    },
                    create_uid: 10,
                    initial_level: 1,
                    thresholds: vec![
                        FieldThreshold {
                            level: 1,
                            progress: 0,
                            definition: FieldDefinition {
                                field_id: 30001,
                                duration: 3,
                            },
                        },
                        FieldThreshold {
                            level: 2,
                            progress: 90,
                            definition: FieldDefinition {
                                field_id: 30002,
                                duration: 3,
                            },
                        },
                    ],
                },
            })
            .unwrap();

        let changes = execute(
            &mut managers,
            FieldTransferCommand {
                origin: ORIGIN,
                target_uid: 10,
                buff: BuffSelector::IdOrType(90071),
                limit: 10,
                team: 1,
            },
        )
        .unwrap();

        assert_eq!(managers.buff.buff_id_or_type_amount(10, 90071), 0);
        assert_eq!(managers.field.get(1).unwrap().progress, 6);
        assert_eq!(managers.field.round_transfer_count(1), 1);
        assert_eq!(changes.buffs[0].change.removed[0].buff.uid, Some(2));
        assert_eq!(changes.field.applied_delta, 6);
        managers.begin_round();
        assert_eq!(managers.field.round_transfer_count(1), 0);
    }

    #[test]
    fn transfer_consumes_configured_extra_value_and_adds_it_to_progress() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(100),
                    buffs: vec![
                        BuffInfo {
                            buff_id: Some(90071),
                            uid: Some(2),
                            layer: Some(6),
                            ..Default::default()
                        },
                        BuffInfo {
                            buff_id: Some(31430141),
                            uid: Some(3),
                            layer: Some(5),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        managers
            .execute_field(FieldCommand {
                origin: ORIGIN,
                team: 1,
                operation: FieldOperation::DeployIfAbsent {
                    definition: FieldDefinition {
                        field_id: 30001,
                        duration: 3,
                    },
                    create_uid: 10,
                    initial_level: 1,
                    thresholds: vec![
                        FieldThreshold {
                            level: 1,
                            progress: 0,
                            definition: FieldDefinition {
                                field_id: 30001,
                                duration: 3,
                            },
                        },
                        FieldThreshold {
                            level: 2,
                            progress: 90,
                            definition: FieldDefinition {
                                field_id: 30002,
                                duration: 3,
                            },
                        },
                    ],
                },
            })
            .unwrap();

        let changes = execute(
            &mut managers,
            FieldTransferCommand {
                origin: ORIGIN,
                target_uid: 10,
                buff: BuffSelector::IdOrType(90071),
                limit: 10,
                team: 1,
            },
        )
        .unwrap();

        assert_eq!(managers.buff.buff_id_or_type_amount(10, 90071), 0);
        assert_eq!(managers.buff.buff_id_or_type_amount(10, 31430141), 0);
        assert_eq!(managers.field.get(1).unwrap().progress, 11);
        assert_eq!(changes.buffs.len(), 2);
        assert_eq!(changes.field.applied_delta, 11);
    }

    #[test]
    fn allied_extra_transfer_records_and_snapshots_team_attributes() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(10),
                        team_type: Some(1),
                        current_hp: Some(100),
                        buffs: vec![
                            BuffInfo {
                                buff_id: Some(31430146),
                                uid: Some(4),
                                ..Default::default()
                            },
                            BuffInfo {
                                buff_id: Some(90071),
                                uid: Some(5),
                                layer: Some(6),
                                ..Default::default()
                            },
                            BuffInfo {
                                buff_id: Some(31430141),
                                uid: Some(6),
                                layer: Some(5),
                                ..Default::default()
                            },
                        ],
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(20),
                        team_type: Some(1),
                        current_hp: Some(100),
                        buffs: vec![
                            BuffInfo {
                                buff_id: Some(90071),
                                uid: Some(2),
                                layer: Some(6),
                                ..Default::default()
                            },
                            BuffInfo {
                                buff_id: Some(31430141),
                                uid: Some(3),
                                layer: Some(5),
                                ..Default::default()
                            },
                        ],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        managers
            .execute_field(FieldCommand {
                origin: ORIGIN,
                team: 1,
                operation: FieldOperation::DeployIfAbsent {
                    definition: FieldDefinition {
                        field_id: 30001,
                        duration: 3,
                    },
                    create_uid: 10,
                    initial_level: 1,
                    thresholds: vec![
                        FieldThreshold {
                            level: 1,
                            progress: 0,
                            definition: FieldDefinition {
                                field_id: 30001,
                                duration: 3,
                            },
                        },
                        FieldThreshold {
                            level: 2,
                            progress: 90,
                            definition: FieldDefinition {
                                field_id: 30002,
                                duration: 3,
                            },
                        },
                    ],
                },
            })
            .unwrap();

        execute(
            &mut managers,
            FieldTransferCommand {
                origin: ORIGIN,
                target_uid: 10,
                buff: BuffSelector::IdOrType(90071),
                limit: 10,
                team: 1,
            },
        )
        .unwrap();
        assert_eq!(managers.buff.act_value(4, 1128), 0);

        execute(
            &mut managers,
            FieldTransferCommand {
                origin: ORIGIN,
                target_uid: 20,
                buff: BuffSelector::IdOrType(90071),
                limit: 10,
                team: 1,
            },
        )
        .unwrap();
        assert_eq!(managers.buff.act_value(4, 1128), 5);

        let changes = managers
            .execute_buff(BuffCommand::Grant(BuffGrant {
                origin: ORIGIN,
                source_uid: 10,
                target_uid: 10,
                buff_id: 31430131,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }))
            .unwrap();
        let added = changes.change.added.unwrap();
        assert_eq!(added.buff.act_info[0].act_id, Some(1127));
        assert_eq!(added.buff.act_info[0].param, vec![100]);
        for uid in [10, 20] {
            assert_eq!(managers.buff.attribute_delta(uid, AttrId::DmgBonus), 100);
            assert_eq!(managers.buff.attribute_delta(uid, AttrId::CriticalDmg), 200);
        }
    }
}
