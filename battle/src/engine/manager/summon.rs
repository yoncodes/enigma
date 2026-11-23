use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SummonApplyResult {
    pub target_uid: i64,
    pub summoned_id: i32,
    pub level: i32,
    pub uid: i64,
    pub from_uid: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SummonState {
    pub count: i32,
    pub level: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummonOperation {
    Add {
        target_uid: i64,
        count: i32,
        level: i32,
    },
    ChangeLevel {
        level: i32,
    },
    AddLevel {
        delta: i32,
    },
    Remove {
        count: i32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SummonCommand {
    pub origin: crate::engine::skill::rule::CommandOrigin,
    pub owner_uid: i64,
    pub summoned_id: i32,
    pub operation: SummonOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SummonChanges {
    pub origin: crate::engine::skill::rule::CommandOrigin,
    pub owner_uid: i64,
    pub summoned_id: i32,
    pub before: Option<SummonState>,
    pub after: Option<SummonState>,
    pub target_uid: i64,
    pub operation: SummonOperation,
}

impl SummonChanges {
    pub fn events(self) -> Vec<crate::engine::event::payload::BattleEvent> {
        (self.before != self.after)
            .then_some(crate::engine::event::payload::BattleEvent::SummonChanged(
                crate::engine::event::payload::SummonChangeEvent {
                    origin: self.origin,
                    owner_uid: self.owner_uid,
                    summoned_id: self.summoned_id,
                    before_count: self.before.map(|state| state.count).unwrap_or_default(),
                    after_count: self.after.map(|state| state.count).unwrap_or_default(),
                    before_level: self.before.map(|state| state.level).unwrap_or_default(),
                    after_level: self.after.map(|state| state.level).unwrap_or_default(),
                },
            ))
            .into_iter()
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummonCommandError {
    InvalidCommand,
    MissingSummon,
}

#[derive(Debug, Clone, Default)]
pub struct SummonManager {
    active: HashMap<(i64, i32), SummonState>,
}

impl SummonManager {
    pub fn get(&self, owner_uid: i64, summoned_id: i32) -> Option<SummonState> {
        self.active.get(&(owner_uid, summoned_id)).copied()
    }

    pub(crate) fn execute_command(
        &mut self,
        command: SummonCommand,
    ) -> Result<SummonChanges, SummonCommandError> {
        if command.owner_uid == 0 || command.summoned_id <= 0 {
            return Err(SummonCommandError::InvalidCommand);
        }
        let before = self.get(command.owner_uid, command.summoned_id);
        let target_uid = match command.operation {
            SummonOperation::Add {
                target_uid,
                count,
                level,
            } => {
                if count <= 0 {
                    return Err(SummonCommandError::InvalidCommand);
                }
                let state = self
                    .active
                    .entry((command.owner_uid, command.summoned_id))
                    .or_insert(SummonState {
                        count,
                        level: level.max(1),
                    });
                state.count = state.count.max(count);
                state.level = state.level.max(level.max(1));
                target_uid
            }
            SummonOperation::ChangeLevel { level } => {
                self.active
                    .get_mut(&(command.owner_uid, command.summoned_id))
                    .ok_or(SummonCommandError::MissingSummon)?
                    .level = level.max(0);
                command.owner_uid
            }
            SummonOperation::AddLevel { delta } => {
                if delta == 0 {
                    return Err(SummonCommandError::InvalidCommand);
                }
                let state = self
                    .active
                    .get_mut(&(command.owner_uid, command.summoned_id))
                    .ok_or(SummonCommandError::MissingSummon)?;
                state.level = state.level.saturating_add(delta).max(0);
                command.owner_uid
            }
            SummonOperation::Remove { count } => {
                if !self.remove(command.owner_uid, command.summoned_id, count) {
                    return Err(SummonCommandError::MissingSummon);
                }
                command.owner_uid
            }
        };
        Ok(SummonChanges {
            origin: command.origin,
            owner_uid: command.owner_uid,
            summoned_id: command.summoned_id,
            before,
            after: self.get(command.owner_uid, command.summoned_id),
            target_uid,
            operation: command.operation,
        })
    }
    pub fn active_unique_skills(&self, game_data: &config::GameDB) -> Vec<(i64, i32)> {
        self.skill_owners(|summoned_id| {
            crate::catalog::summoned_unique_skills(game_data, summoned_id)
        })
    }

    pub(crate) fn skill_owners(&self, mut skills: impl FnMut(i32) -> Vec<i32>) -> Vec<(i64, i32)> {
        let mut active = self.active.keys().copied().collect::<Vec<_>>();
        active.sort_by_key(|(owner_uid, summoned_id)| (*owner_uid, -summoned_lane(*summoned_id)));
        active
            .into_iter()
            .flat_map(|(owner_uid, summoned_id)| {
                skills(summoned_id)
                    .into_iter()
                    .map(move |skill_id| (owner_uid, skill_id))
            })
            .collect()
    }

    pub fn count(&self, owner_uid: i64, summoned_id: i32, required_level: i32) -> i32 {
        self.active
            .get(&(owner_uid, summoned_id))
            .filter(|state| required_level <= 0 || state.level == required_level)
            .map(|state| state.count)
            .unwrap_or_default()
    }

    pub fn total(&self, owner_uid: i64, required_level: i32) -> i32 {
        self.active
            .iter()
            .filter(|((uid, _), state)| {
                *uid == owner_uid && (required_level <= 0 || state.level == required_level)
            })
            .map(|(_, state)| state.count)
            .sum()
    }

    pub fn add(
        &mut self,
        source_uid: i64,
        target_uid: i64,
        summoned_id: i32,
        count: i32,
        level: i32,
    ) -> Vec<SummonApplyResult> {
        if source_uid == 0 || summoned_id == 0 || count <= 0 {
            return Vec::new();
        }
        let key = (source_uid, summoned_id);
        if let Some(current) = self.active.get_mut(&key) {
            current.count = current.count.max(count);
            current.level = current.level.max(level.max(1));
            return Vec::new();
        }
        self.active.insert(
            key,
            SummonState {
                count,
                level: level.max(1),
            },
        );
        vec![SummonApplyResult {
            target_uid,
            summoned_id,
            level: level.max(1),
            uid: summoned_lane(summoned_id) as i64,
            from_uid: source_uid,
        }]
    }

    pub fn change_level(
        &mut self,
        source_uid: i64,
        summoned_id: i32,
        level: i32,
    ) -> Option<SummonApplyResult> {
        let state = self.active.get_mut(&(source_uid, summoned_id))?;
        state.level = level.max(0);
        Some(result(source_uid, summoned_id, state.level))
    }

    pub fn add_level(
        &mut self,
        source_uid: i64,
        summoned_id: i32,
        delta: i32,
    ) -> Option<SummonApplyResult> {
        if delta == 0 {
            return None;
        }
        let state = self.active.get_mut(&(source_uid, summoned_id))?;
        state.level = (state.level + delta).max(0);
        Some(result(source_uid, summoned_id, state.level))
    }

    pub fn remove(&mut self, source_uid: i64, summoned_id: i32, count: i32) -> bool {
        let key = (source_uid, summoned_id);
        let Some(state) = self.active.get_mut(&key) else {
            return false;
        };
        if count <= 0 || state.count <= count {
            self.active.remove(&key);
        } else {
            state.count -= count;
        }
        true
    }
}

fn result(source_uid: i64, summoned_id: i32, level: i32) -> SummonApplyResult {
    SummonApplyResult {
        target_uid: source_uid,
        summoned_id,
        level,
        uid: summoned_lane(summoned_id) as i64,
        from_uid: source_uid,
    }
}

pub fn summoned_lane(model_id: i32) -> i32 {
    if model_id > 0 {
        model_id / 10 % 10 + 1
    } else {
        0
    }
}

#[cfg(test)]
mod tests;
