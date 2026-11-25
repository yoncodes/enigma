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

    pub(super) fn sync(&self, fight: &mut Fight) {
        fight.progress = self.value;
        fight.progress_max = self.max;
    }
}
