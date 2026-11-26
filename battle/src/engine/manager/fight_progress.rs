use sonettobuf::{Fight, FightProgressInfo};

use crate::engine::skill::rule::CommandOrigin;

mod legacy;

use legacy::LegacyProgress;
pub use legacy::{LegacyProgressAdd, LegacyProgressClear, LegacyProgressDelta, LegacyProgressInit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FightProgressState {
    pub id: i32,
    pub max: i32,
    pub value: i32,
    pub show_id: i32,
    pub skill_id: i32,
}

impl From<FightProgressState> for FightProgressInfo {
    fn from(state: FightProgressState) -> Self {
        Self {
            id: Some(state.id),
            max: Some(state.max),
            value: Some(state.value),
            show_id: Some(state.show_id),
            skill_id: Some(state.skill_id),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FightProgressCommand {
    pub origin: CommandOrigin,
    pub initial: i32,
    pub max: i32,
    pub show_id: i32,
    pub progress_id: i32,
    pub skill_id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FightProgressChange {
    pub origin: CommandOrigin,
    pub state: FightProgressState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FightProgressValueCommand {
    pub origin: CommandOrigin,
    pub progress_id: i32,
    pub delta: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FightProgressValueChange {
    pub origin: CommandOrigin,
    pub before: FightProgressState,
    pub after: FightProgressState,
    pub delta: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FightProgressError {
    InvalidDefinition,
    AlreadyInitialized(i32),
    MissingProgress(i32),
    AmbiguousProgress(i32),
    InvalidStoredProgress(i32),
}

#[derive(Debug, Clone, Default)]
pub struct FightProgressManager {
    legacy: LegacyProgress,
    records: Vec<FightProgressInfo>,
}

impl FightProgressManager {
    pub fn seed(&mut self, fight: &Fight) {
        self.legacy = LegacyProgress::seed(fight);
        self.records.clone_from(&fight.progress_list);
    }

    pub(crate) fn initialize_legacy(
        &mut self,
        command: LegacyProgressInit,
    ) -> Result<FightProgressChange, FightProgressError> {
        self.legacy.initialize(command)
    }

    pub(crate) fn legacy_is_full(&self) -> bool {
        self.legacy.is_full()
    }

    pub(crate) fn add_legacy(
        &mut self,
        command: LegacyProgressAdd,
    ) -> Result<Option<LegacyProgressDelta>, FightProgressError> {
        self.legacy.add(command)
    }

    pub(crate) fn clear_legacy(
        &mut self,
    ) -> Result<Option<LegacyProgressDelta>, FightProgressError> {
        self.legacy.clear()
    }

    pub fn initialize(
        &mut self,
        command: FightProgressCommand,
    ) -> Result<FightProgressChange, FightProgressError> {
        if command.progress_id <= 0
            || command.max <= 0
            || !(0..=command.max).contains(&command.initial)
            || command.show_id <= 0
            || command.skill_id <= 0
        {
            return Err(FightProgressError::InvalidDefinition);
        }
        if self
            .records
            .iter()
            .any(|record| record.id == Some(command.progress_id))
        {
            return Err(FightProgressError::AlreadyInitialized(command.progress_id));
        }
        let state = FightProgressState {
            id: command.progress_id,
            max: command.max,
            value: command.initial,
            show_id: command.show_id,
            skill_id: command.skill_id,
        };
        self.records.push(state.into());
        Ok(FightProgressChange {
            origin: command.origin,
            state,
        })
    }

    pub fn get(&self, progress_id: i32) -> Option<FightProgressState> {
        let record = self
            .records
            .iter()
            .find(|record| record.id == Some(progress_id))?;
        Some(FightProgressState {
            id: record.id?,
            max: record.max?,
            value: record.value?,
            show_id: record.show_id?,
            skill_id: record.skill_id?,
        })
    }

    pub fn change_value(
        &mut self,
        command: FightProgressValueCommand,
    ) -> Result<Option<FightProgressValueChange>, FightProgressError> {
        let mut matches = self
            .records
            .iter()
            .enumerate()
            .filter(|(_, record)| record.id == Some(command.progress_id));
        let index = matches
            .next()
            .map(|(index, _)| index)
            .ok_or(FightProgressError::MissingProgress(command.progress_id))?;
        if matches.next().is_some() {
            return Err(FightProgressError::AmbiguousProgress(command.progress_id));
        }
        let record = &mut self.records[index];
        let before = FightProgressState {
            id: record.id.ok_or(FightProgressError::InvalidStoredProgress(
                command.progress_id,
            ))?,
            max: record.max.ok_or(FightProgressError::InvalidStoredProgress(
                command.progress_id,
            ))?,
            value: record
                .value
                .ok_or(FightProgressError::InvalidStoredProgress(
                    command.progress_id,
                ))?,
            show_id: record
                .show_id
                .ok_or(FightProgressError::InvalidStoredProgress(
                    command.progress_id,
                ))?,
            skill_id: record
                .skill_id
                .ok_or(FightProgressError::InvalidStoredProgress(
                    command.progress_id,
                ))?,
        };
        if before.max <= 0 || !(0..=before.max).contains(&before.value) {
            return Err(FightProgressError::InvalidStoredProgress(
                command.progress_id,
            ));
        }
        let value = before
            .value
            .saturating_add(command.delta)
            .clamp(0, before.max);
        let after = FightProgressState { value, ..before };
        record.value = Some(after.value);
        if after == before {
            return Ok(None);
        }
        Ok(Some(FightProgressValueChange {
            origin: command.origin,
            before,
            after,
            delta: after.value - before.value,
        }))
    }

    pub fn sync_fight(&self, fight: &mut Fight) {
        self.legacy.sync(fight);
        fight.progress_list.clone_from(&self.records);
    }
}

#[cfg(test)]
mod tests;
