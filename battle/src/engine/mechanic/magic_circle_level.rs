use crate::engine::{
    manager::{
        BattleManagers,
        buff::{
            BuffChanges, BuffCommand, BuffCommandError, BuffGrant, BuffRemove, BuffRemoveSelector,
        },
        emitter,
        field::{FieldChange, FieldCommand, FieldCommandError, FieldDefinition, FieldOperation},
    },
    skill::rule::CommandOrigin,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MagicCircleLevelCommand {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub team: i32,
    pub expected_field_id: i32,
    pub replacement: FieldDefinition,
    pub level: i32,
    pub removed_buffs: Vec<i32>,
    pub added_buffs: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MagicCircleLevelChanges {
    pub removed_buffs: Vec<BuffChanges>,
    pub removed_field: FieldChange,
    pub added_buffs: Vec<BuffChanges>,
    pub added_field: FieldChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MagicCircleLevelError {
    InvalidCommand,
    MissingField(i32),
    UnexpectedField { expected: i32, actual: i32 },
    Buff(BuffCommandError),
    Field(FieldCommandError),
}

impl From<BuffCommandError> for MagicCircleLevelError {
    fn from(value: BuffCommandError) -> Self {
        Self::Buff(value)
    }
}

impl From<FieldCommandError> for MagicCircleLevelError {
    fn from(value: FieldCommandError) -> Self {
        Self::Field(value)
    }
}

pub(crate) fn execute(
    managers: &mut BattleManagers,
    command: MagicCircleLevelCommand,
) -> Result<MagicCircleLevelChanges, MagicCircleLevelError> {
    if command.source_uid == 0
        || command.team == 0
        || command.expected_field_id <= 0
        || command.replacement.field_id <= 0
        || command.replacement.duration < -1
        || command.level <= 0
    {
        return Err(MagicCircleLevelError::InvalidCommand);
    }
    let current = managers
        .field
        .get(command.team)
        .ok_or(MagicCircleLevelError::MissingField(command.team))?;
    if current.definition.field_id != command.expected_field_id {
        return Err(MagicCircleLevelError::UnexpectedField {
            expected: command.expected_field_id,
            actual: current.definition.field_id,
        });
    }

    let mut next = managers.clone();
    let mut removed_buffs = Vec::new();
    for buff_id in command.removed_buffs {
        if next.buff.has_buff_id(emitter::UID, buff_id) {
            removed_buffs.push(next.execute_buff(BuffCommand::Remove(BuffRemove {
                origin: command.origin,
                target_uid: emitter::UID,
                selector: BuffRemoveSelector::ExactId(buff_id),
            }))?);
        }
    }
    let removed_field = next.execute_field(FieldCommand {
        origin: command.origin,
        team: command.team,
        operation: FieldOperation::Remove,
    })?;

    let mut added_buffs = Vec::new();
    for buff_id in command.added_buffs {
        added_buffs.push(next.execute_buff(BuffCommand::Grant(BuffGrant {
            origin: command.origin,
            source_uid: command.source_uid,
            target_uid: emitter::UID,
            buff_id,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))?);
    }
    let added_field = next.execute_field(FieldCommand {
        origin: command.origin,
        team: command.team,
        operation: FieldOperation::DeployReplacement {
            definition: command.replacement,
            owner_uid: command.source_uid,
            create_uid: -1,
            level: command.level,
        },
    })?;

    *managers = next;
    Ok(MagicCircleLevelChanges {
        removed_buffs,
        removed_field,
        added_buffs,
        added_field,
    })
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    use super::*;
    use crate::engine::skill::rule::{DefinitionKey, RuleDomain};

    const ORIGIN: CommandOrigin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60270, "UpdateWangQiMagicCircle"),
    };

    #[test]
    fn replaces_linked_buffs_and_field_as_one_transaction() {
        crate::test_support::init_config();
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(7),
                    team_type: Some(1),
                    ..Default::default()
                }],
                sub_entitys: vec![FightEntityInfo {
                    uid: Some(emitter::UID),
                    team_type: Some(1),
                    buffs: [31410601, 31412602, 31410603]
                        .into_iter()
                        .enumerate()
                        .map(|(index, buff_id)| BuffInfo {
                            buff_id: Some(buff_id),
                            uid: Some(index as i64 + 1),
                            from_uid: Some(7),
                            ..Default::default()
                        })
                        .collect(),
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
                        field_id: 42003,
                        duration: -1,
                    },
                    create_uid: 7,
                    initial_level: 3,
                    thresholds: Vec::new(),
                },
            })
            .unwrap();

        let changes = execute(
            &mut managers,
            MagicCircleLevelCommand {
                origin: ORIGIN,
                source_uid: 7,
                team: 1,
                expected_field_id: 42003,
                replacement: FieldDefinition {
                    field_id: 42004,
                    duration: 4,
                },
                level: 4,
                removed_buffs: vec![31410601, 31412602, 31410603],
                added_buffs: vec![31410601, 31412602, 31410603, 31410604],
            },
        )
        .unwrap();

        assert_eq!(changes.removed_buffs.len(), 3);
        assert_eq!(changes.added_buffs.len(), 4);
        assert_eq!(
            changes.removed_field.before.unwrap().definition.field_id,
            42003
        );
        let field = changes.added_field.after.unwrap();
        assert_eq!(field.definition.field_id, 42004);
        assert_eq!(field.owner_uid, 7);
        assert_eq!(field.create_uid, -1);
        assert_eq!(field.level, 4);
        for buff_id in [31410601, 31412602, 31410603, 31410604] {
            assert!(managers.buff.has_buff_id(emitter::UID, buff_id));
        }
    }
}
