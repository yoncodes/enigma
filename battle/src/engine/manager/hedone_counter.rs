use std::collections::{HashMap, VecDeque};

use crate::engine::skill::rule::CommandOrigin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HedoneCounterRecord {
    pub origin: CommandOrigin,
    pub buff_uid: i64,
    pub visible: i32,
    pub amount: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HedoneCounterSettle {
    pub origin: CommandOrigin,
    pub buff_uid: i64,
    pub visible: i32,
    pub retained_rounds: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HedoneCounterCommand {
    Record(HedoneCounterRecord),
    Settle(HedoneCounterSettle),
    Clear {
        origin: CommandOrigin,
        buff_uid: i64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HedoneCounterError {
    InvalidRetainedRounds(i32),
    MissingHistory { buff_uid: i64, visible: i32 },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CounterState {
    completed: VecDeque<i32>,
    current: i32,
}

impl CounterState {
    fn total(&self) -> i32 {
        self.completed
            .iter()
            .copied()
            .fold(self.current, i32::saturating_add)
    }

    fn settle(&mut self, retained_rounds: usize) -> i32 {
        self.completed.push_back(std::mem::take(&mut self.current));
        while self.completed.len() > retained_rounds {
            self.completed.pop_front();
        }
        self.total()
    }
}

#[derive(Debug, Clone, Default)]
pub struct HedoneCounterManager {
    counters: HashMap<i64, CounterState>,
}

impl HedoneCounterManager {
    pub fn value(&self, buff_uid: i64, visible: i32) -> Result<i32, HedoneCounterError> {
        match self.counters.get(&buff_uid) {
            Some(state) => Ok(state.total()),
            None if visible == 0 => Ok(0),
            None => Err(HedoneCounterError::MissingHistory { buff_uid, visible }),
        }
    }

    pub fn settled_value(&self, settle: HedoneCounterSettle) -> Result<i32, HedoneCounterError> {
        let mut state = self.state(settle.buff_uid, settle.visible)?.clone();
        Ok(state.settle(settle.retained_rounds))
    }

    pub fn execute(&mut self, command: HedoneCounterCommand) -> Result<(), HedoneCounterError> {
        match command {
            HedoneCounterCommand::Record(record) => {
                let state = self.state_mut(record.buff_uid, record.visible)?;
                state.current = state.current.saturating_add(record.amount.max(0));
            }
            HedoneCounterCommand::Settle(settle) => {
                self.state_mut(settle.buff_uid, settle.visible)?
                    .settle(settle.retained_rounds);
            }
            HedoneCounterCommand::Clear { buff_uid, .. } => {
                self.counters.remove(&buff_uid);
            }
        }
        Ok(())
    }

    fn state(&self, buff_uid: i64, visible: i32) -> Result<&CounterState, HedoneCounterError> {
        match self.counters.get(&buff_uid) {
            Some(state) => Ok(state),
            None if visible == 0 => Ok(&EMPTY),
            None => Err(HedoneCounterError::MissingHistory { buff_uid, visible }),
        }
    }

    fn state_mut(
        &mut self,
        buff_uid: i64,
        visible: i32,
    ) -> Result<&mut CounterState, HedoneCounterError> {
        if !self.counters.contains_key(&buff_uid) && visible != 0 {
            return Err(HedoneCounterError::MissingHistory { buff_uid, visible });
        }
        Ok(self.counters.entry(buff_uid).or_default())
    }
}

static EMPTY: CounterState = CounterState {
    completed: VecDeque::new(),
    current: 0,
};

#[cfg(test)]
mod tests;
