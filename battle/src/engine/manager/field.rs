use std::collections::HashMap;

use crate::engine::{
    entity::attr::AttrId,
    event::payload::BattleEvent,
    skill::{rule::CommandOrigin, target::TargetPool},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldDefinition {
    pub field_id: i32,
    pub duration: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldThreshold {
    pub level: i32,
    pub progress: i32,
    pub definition: FieldDefinition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldState {
    pub origin: CommandOrigin,
    pub definition: FieldDefinition,
    pub team: i32,
    pub create_uid: i64,
    pub level: i32,
    pub progress: i32,
    pub next_upgrade_progress: i32,
    pub terminal_progress: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldOperation {
    DeployIfAbsent {
        definition: FieldDefinition,
        create_uid: i64,
        initial_level: i32,
        thresholds: Vec<FieldThreshold>,
    },
    ChangeProgress {
        delta: i32,
    },
    ChangeDuration {
        delta: i32,
    },
    ResolveLevel {
        thresholds: Vec<FieldThreshold>,
    },
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldCommand {
    pub origin: CommandOrigin,
    pub team: i32,
    pub operation: FieldOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldChangeKind {
    Deployed,
    Progress,
    Duration,
    Level,
    Removed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldChange {
    pub origin: CommandOrigin,
    pub team: i32,
    pub kind: FieldChangeKind,
    pub before: Option<FieldState>,
    pub after: Option<FieldState>,
    pub requested_delta: i32,
    pub applied_delta: i32,
    pub overflow: i32,
}

impl FieldChange {
    pub fn events(self) -> Vec<BattleEvent> {
        (self.before != self.after)
            .then_some(BattleEvent::FieldChanged(
                crate::engine::event::payload::FieldChangeEvent {
                    origin: self.origin,
                    team: self.team,
                    kind: self.kind,
                    field_id: self
                        .after
                        .or(self.before)
                        .map(|state| state.definition.field_id)
                        .unwrap_or_default(),
                    before_level: self.before.map(|state| state.level).unwrap_or_default(),
                    after_level: self.after.map(|state| state.level).unwrap_or_default(),
                    before_progress: self.before.map(|state| state.progress).unwrap_or_default(),
                    after_progress: self.after.map(|state| state.progress).unwrap_or_default(),
                    overflow: self.overflow,
                },
            ))
            .into_iter()
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldCommandError {
    InvalidCommand,
    MissingField(i32),
}

#[derive(Debug, Clone, Default)]
/// Owns team-scoped field identity, level, progress, duration, and transfer state.
/// `create_uid` is protocol attribution, not the scope for allied field rules.
pub struct FieldManager {
    states: HashMap<i32, FieldState>,
    round_transfers: HashMap<i32, i32>,
}

pub(crate) struct FieldPlan {
    next: FieldManager,
    change: FieldChange,
}

impl FieldManager {
    pub fn get(&self, team: i32) -> Option<FieldState> {
        self.states.get(&team).copied()
    }

    pub(crate) fn states(&self) -> impl Iterator<Item = FieldState> + '_ {
        self.states.values().copied()
    }

    pub(crate) fn record_transfer(&mut self, team: i32) {
        if team != 0 {
            *self.round_transfers.entry(team).or_default() += 1;
        }
    }

    pub(crate) fn round_transfer_count(&self, team: i32) -> i32 {
        self.round_transfers.get(&team).copied().unwrap_or_default()
    }

    pub(crate) fn begin_round(&mut self) {
        self.round_transfers.clear();
    }

    pub fn attribute_delta(
        &self,
        catalog: crate::catalog::BattleCatalog,
        uid: i64,
        attr_id: AttrId,
        pool: &TargetPool,
    ) -> i32 {
        let Some(entity_team) = pool.team_type(uid) else {
            return 0;
        };
        self.states()
            .filter_map(|state| {
                let definition = catalog.magic_circle(state.definition.field_id)?;
                Some(if state.team == entity_team {
                    definition.allied_attributes
                } else {
                    definition.enemy_attributes
                })
            })
            .flatten()
            .filter_map(|(configured_attr, delta)| {
                (configured_attr == attr_id as i32).then_some(delta)
            })
            .sum()
    }

    pub(crate) fn plan_command(
        &self,
        command: FieldCommand,
    ) -> Result<FieldPlan, FieldCommandError> {
        // fields are team-scoped and tiny; replace the clone with an
        // operation plan only if profiling shows this transaction boundary matters.
        let mut next = self.clone();
        let change = next.apply_command(command)?;
        Ok(FieldPlan { next, change })
    }

    pub(crate) fn commit(&mut self, plan: FieldPlan) -> FieldChange {
        *self = plan.next;
        plan.change
    }

    pub(crate) fn execute_command(
        &mut self,
        command: FieldCommand,
    ) -> Result<FieldChange, FieldCommandError> {
        let plan = self.plan_command(command)?;
        Ok(self.commit(plan))
    }

    fn apply_command(&mut self, command: FieldCommand) -> Result<FieldChange, FieldCommandError> {
        if command.team == 0 {
            return Err(FieldCommandError::InvalidCommand);
        }
        let before = self.get(command.team);
        let (kind, requested_delta, overflow) = match command.operation {
            FieldOperation::DeployIfAbsent {
                definition,
                create_uid,
                initial_level,
                mut thresholds,
            } => {
                if definition.field_id <= 0 || create_uid == 0 || definition.duration < 0 {
                    return Err(FieldCommandError::InvalidCommand);
                }
                validate_thresholds(&thresholds)?;
                thresholds.sort_by_key(|threshold| (threshold.progress, threshold.level));
                let initial_level = initial_level.max(0);
                self.states.entry(command.team).or_insert(FieldState {
                    origin: command.origin,
                    definition,
                    team: command.team,
                    create_uid,
                    level: initial_level,
                    progress: 0,
                    next_upgrade_progress: next_upgrade_progress(&thresholds, initial_level),
                    terminal_progress: terminal_progress(&thresholds),
                });
                (FieldChangeKind::Deployed, 0, 0)
            }
            FieldOperation::ChangeProgress { delta } => {
                if delta == 0 {
                    return Err(FieldCommandError::InvalidCommand);
                }
                let state = self
                    .states
                    .get_mut(&command.team)
                    .ok_or(FieldCommandError::MissingField(command.team))?;
                let before_progress = state.progress;
                state.progress = state.progress.saturating_add(delta).max(0);
                let overflow = (state.progress - state.terminal_progress).max(0)
                    - (before_progress - state.terminal_progress).max(0);
                (FieldChangeKind::Progress, delta, overflow.max(0))
            }
            FieldOperation::ChangeDuration { delta } => {
                if delta == 0 {
                    return Err(FieldCommandError::InvalidCommand);
                }
                let state = self
                    .states
                    .get_mut(&command.team)
                    .ok_or(FieldCommandError::MissingField(command.team))?;
                state.definition.duration = state.definition.duration.saturating_add(delta).max(0);
                (FieldChangeKind::Duration, delta, 0)
            }
            FieldOperation::ResolveLevel { mut thresholds } => {
                if thresholds.is_empty() {
                    return Err(FieldCommandError::InvalidCommand);
                }
                validate_thresholds(&thresholds)?;
                thresholds.sort_by_key(|threshold| (threshold.progress, threshold.level));
                let state = self
                    .states
                    .get_mut(&command.team)
                    .ok_or(FieldCommandError::MissingField(command.team))?;
                if let Some(threshold) = thresholds
                    .iter()
                    .filter(|threshold| state.progress >= threshold.progress)
                    .max_by_key(|threshold| threshold.level)
                    && threshold.level > state.level
                {
                    state.level = threshold.level;
                    state.definition = threshold.definition;
                }
                state.next_upgrade_progress = next_upgrade_progress(&thresholds, state.level);
                state.terminal_progress = terminal_progress(&thresholds);
                (FieldChangeKind::Level, 0, 0)
            }
            FieldOperation::Remove => {
                self.states
                    .remove(&command.team)
                    .ok_or(FieldCommandError::MissingField(command.team))?;
                (FieldChangeKind::Removed, 0, 0)
            }
        };
        let after = self.get(command.team);
        let applied_delta = match kind {
            FieldChangeKind::Progress => {
                after.map(|state| state.progress).unwrap_or_default()
                    - before.map(|state| state.progress).unwrap_or_default()
            }
            FieldChangeKind::Duration => {
                after
                    .map(|state| state.definition.duration)
                    .unwrap_or_default()
                    - before
                        .map(|state| state.definition.duration)
                        .unwrap_or_default()
            }
            FieldChangeKind::Deployed | FieldChangeKind::Level | FieldChangeKind::Removed => 0,
        };
        Ok(FieldChange {
            origin: command.origin,
            team: command.team,
            kind,
            before,
            after,
            requested_delta,
            applied_delta,
            overflow,
        })
    }
}

fn validate_thresholds(thresholds: &[FieldThreshold]) -> Result<(), FieldCommandError> {
    if thresholds.iter().any(|threshold| {
        threshold.level <= 0
            || threshold.progress < 0
            || threshold.definition.field_id <= 0
            || threshold.definition.duration < 0
    }) {
        return Err(FieldCommandError::InvalidCommand);
    }
    Ok(())
}

fn next_upgrade_progress(thresholds: &[FieldThreshold], level: i32) -> i32 {
    thresholds
        .iter()
        .find(|threshold| threshold.level > level)
        .or_else(|| thresholds.last())
        .map(|threshold| threshold.progress)
        .unwrap_or_default()
}

fn terminal_progress(thresholds: &[FieldThreshold]) -> i32 {
    thresholds
        .last()
        .map(|threshold| threshold.progress)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::skill::rule::{DefinitionKey, RuleDomain};

    const ORIGIN: CommandOrigin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(50019, "AddMagicCircle"),
    };

    #[test]
    fn first_deployer_owns_unclamped_progress_and_configured_level_resolution() {
        let mut manager = FieldManager::default();
        let deploy = |create_uid| FieldCommand {
            origin: ORIGIN,
            team: 1,
            operation: FieldOperation::DeployIfAbsent {
                definition: FieldDefinition {
                    field_id: 30001,
                    duration: 3,
                },
                create_uid,
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
                    FieldThreshold {
                        level: 3,
                        progress: 120,
                        definition: FieldDefinition {
                            field_id: 30003,
                            duration: 2,
                        },
                    },
                ],
            },
        };
        manager.execute_command(deploy(10)).unwrap();
        manager.execute_command(deploy(11)).unwrap();
        assert_eq!(manager.get(1).unwrap().create_uid, 10);
        assert_eq!(manager.get(1).unwrap().next_upgrade_progress, 90);

        manager
            .execute_command(FieldCommand {
                origin: ORIGIN,
                team: 1,
                operation: FieldOperation::ChangeProgress { delta: 110 },
            })
            .unwrap();
        assert_eq!(manager.get(1).unwrap().progress, 110);

        let thresholds = vec![
            FieldThreshold {
                level: 2,
                progress: 90,
                definition: FieldDefinition {
                    field_id: 30002,
                    duration: 3,
                },
            },
            FieldThreshold {
                level: 3,
                progress: 120,
                definition: FieldDefinition {
                    field_id: 30003,
                    duration: 2,
                },
            },
        ];
        let resolved = manager
            .execute_command(FieldCommand {
                origin: ORIGIN,
                team: 1,
                operation: FieldOperation::ResolveLevel {
                    thresholds: thresholds.clone(),
                },
            })
            .unwrap();
        assert_eq!(resolved.after.unwrap().level, 2);
        assert_eq!(resolved.after.unwrap().definition.field_id, 30002);
        assert_eq!(resolved.after.unwrap().next_upgrade_progress, 120);
        assert_eq!(resolved.overflow, 0);

        let overflow = manager
            .execute_command(FieldCommand {
                origin: ORIGIN,
                team: 1,
                operation: FieldOperation::ChangeProgress { delta: 20 },
            })
            .unwrap();
        assert_eq!(overflow.after.unwrap().progress, 130);
        assert_eq!(overflow.overflow, 10);

        manager
            .execute_command(FieldCommand {
                origin: ORIGIN,
                team: 1,
                operation: FieldOperation::ChangeDuration { delta: -1 },
            })
            .unwrap();
        manager
            .execute_command(FieldCommand {
                origin: ORIGIN,
                team: 1,
                operation: FieldOperation::ResolveLevel { thresholds },
            })
            .unwrap();
        assert_eq!(manager.get(1).unwrap().definition.duration, 2);
    }

    #[test]
    fn active_field_attributes_are_read_from_the_authoritative_team_state() {
        crate::test_support::init_config();
        let pool = TargetPool::from_fight(&sonettobuf::Fight {
            attacker: Some(sonettobuf::FightTeam {
                entitys: vec![sonettobuf::FightEntityInfo {
                    uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(sonettobuf::FightTeam {
                entitys: vec![sonettobuf::FightEntityInfo {
                    uid: Some(-1),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        });
        let mut manager = FieldManager::default();
        manager
            .execute_command(FieldCommand {
                origin: ORIGIN,
                team: 1,
                operation: FieldOperation::DeployIfAbsent {
                    definition: FieldDefinition {
                        field_id: 30001,
                        duration: 3,
                    },
                    create_uid: 10,
                    initial_level: 1,
                    thresholds: Vec::new(),
                },
            })
            .unwrap();

        assert_eq!(
            manager.attribute_delta(
                crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
                10,
                AttrId::DmgBonus,
                &pool
            ),
            150
        );
        assert_eq!(
            manager.attribute_delta(
                crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
                -1,
                AttrId::DmgBonus,
                &pool
            ),
            0
        );
    }
}
