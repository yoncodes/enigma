use sonettobuf::Fight;

use super::{FightProgressChange, FightProgressError, FightProgressState};
use crate::engine::skill::rule::CommandOrigin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyProgressInit {
    pub origin: CommandOrigin,
    pub initial: i32,
    pub max: i32,
    pub show_id: i32,
    pub skill_id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyProgressAdd {
    pub origin: CommandOrigin,
    pub amount: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyProgressGain {
    pub max: i32,
    pub delta: i32,
}

#[derive(Debug, Clone, Default)]
pub(super) struct LegacyProgress {
    value: Option<i32>,
    max: Option<i32>,
}

impl LegacyProgress {
    pub(super) fn seed(fight: &Fight) -> Self {
        Self {
            value: fight.progress,
            max: fight.progress_max,
        }
    }

    pub(super) fn initialize(
        &mut self,
        command: LegacyProgressInit,
    ) -> Result<FightProgressChange, FightProgressError> {
        if command.max <= 0
            || !(0..=command.max).contains(&command.initial)
            || command.show_id <= 0
            || command.skill_id <= 0
        {
            return Err(FightProgressError::InvalidDefinition);
        }
        self.value = Some(command.initial);
        self.max = Some(command.max);
        Ok(FightProgressChange {
            origin: command.origin,
            state: FightProgressState {
                id: 0,
                max: command.max,
                value: command.initial,
                show_id: command.show_id,
                skill_id: command.skill_id,
            },
        })
    }

    pub(super) fn is_full(&self) -> bool {
        self.max
            .is_some_and(|max| max > 0 && self.value == Some(max))
    }

    pub(super) fn sync(&self, fight: &mut Fight) {
        fight.progress = self.value;
        fight.progress_max = self.max;
    }

    pub(super) fn add(
        &mut self,
        command: LegacyProgressAdd,
    ) -> Result<Option<LegacyProgressGain>, FightProgressError> {
        if command.amount <= 0 {
            return Err(FightProgressError::InvalidDefinition);
        }
        let (Some(before), Some(max)) = (self.value, self.max) else {
            return Err(FightProgressError::MissingProgress(0));
        };
        if max <= 0 || !(0..=max).contains(&before) {
            return Err(FightProgressError::InvalidStoredProgress(0));
        }
        let after = before.saturating_add(command.amount).min(max);
        self.value = Some(after);
        Ok((after != before).then_some(LegacyProgressGain {
            max,
            delta: after - before,
        }))
    }
}
