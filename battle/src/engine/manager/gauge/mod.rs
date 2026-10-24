use std::collections::{HashMap, VecDeque};

use crate::engine::{
    event::payload::{BattleEvent, GaugeChangeEvent},
    skill::rule::CommandOrigin,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GaugeKind {
    Bloodtithe,
    LingeringGlow,
    TeamEnergy,
    ImpromptuInspiration,
}

impl GaugeKind {
    pub const fn from_shared_pool_config_effect(config_effect: i32) -> Option<Self> {
        match config_effect {
            0 => Some(Self::Bloodtithe),
            1 => Some(Self::LingeringGlow),
            _ => None,
        }
    }

    pub const fn shared_pool_config_effect(self) -> i32 {
        match self {
            Self::LingeringGlow => 1,
            Self::Bloodtithe | Self::TeamEnergy | Self::ImpromptuInspiration => 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GaugeOwner {
    Team(i32),
    Entity(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GaugeKey {
    pub kind: GaugeKind,
    pub owner: GaugeOwner,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GaugeState {
    pub current: i32,
    pub max: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GaugeOperation {
    Enable {
        max: Option<i32>,
    },
    ChangeValue {
        delta: i32,
    },
    AccumulateRawValue {
        amount: i32,
        stream: i32,
    },
    AccumulateValue {
        amount: i32,
        threshold: i32,
    },
    ChangeMax {
        delta: i32,
    },
    SyncBaseMax {
        max: i32,
    },
    ConsumeAccumulated {
        listener_uid: i64,
        listener_opcode: i32,
        amount: i32,
    },
    AccumulateProgress {
        raw_amount: i32,
    },
    Snapshot,
    Disable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GaugeCommand {
    pub origin: CommandOrigin,
    pub key: GaugeKey,
    pub operation: GaugeOperation,
    pub source_uid: i64,
    pub source_skill_id: i32,
    pub config_effect: i32,
    pub raw_delta: Option<i32>,
    pub progress_raw_delta: Option<i32>,
}

impl GaugeCommand {
    pub const fn new(origin: CommandOrigin, key: GaugeKey, operation: GaugeOperation) -> Self {
        Self {
            origin,
            key,
            operation,
            source_uid: 0,
            source_skill_id: 0,
            config_effect: 0,
            raw_delta: None,
            progress_raw_delta: None,
        }
    }

    pub const fn attributed_to(mut self, source_uid: i64, config_effect: i32) -> Self {
        self.source_uid = source_uid;
        self.config_effect = config_effect;
        self
    }

    pub const fn caused_by_skill(mut self, skill_id: i32) -> Self {
        self.source_skill_id = skill_id;
        self
    }

    pub const fn with_raw_delta(mut self, raw_delta: i32) -> Self {
        self.raw_delta = Some(raw_delta);
        self
    }

    pub const fn with_progress_raw_delta(mut self, raw_delta: i32) -> Self {
        self.progress_raw_delta = Some(raw_delta);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GaugeChangeKind {
    Enabled,
    Value,
    Max,
    Accumulated,
    Snapshot,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GaugeChange {
    pub origin: CommandOrigin,
    pub key: GaugeKey,
    pub source_uid: i64,
    pub source_skill_id: i32,
    pub config_effect: i32,
    pub progress_raw_delta: i32,
    pub kind: GaugeChangeKind,
    pub before: i32,
    pub requested_delta: i32,
    pub applied_delta: i32,
    pub after: i32,
    pub overflow: i32,
    pub before_max: Option<i32>,
    pub after_max: Option<i32>,
    pub enabled_before: bool,
    pub enabled_after: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GaugeBatchChanges {
    pub combined: GaugeChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GaugeBatchId {
    pub origin: CommandOrigin,
    pub key: GaugeKey,
    pub source_uid: i64,
    pub discriminator: i64,
}

impl GaugeBatchId {
    pub const fn new(
        origin: CommandOrigin,
        key: GaugeKey,
        source_uid: i64,
        discriminator: i64,
    ) -> Self {
        Self {
            origin,
            key,
            source_uid,
            discriminator,
        }
    }
}

#[derive(Debug, Clone)]
struct PendingRawGaugeBatch {
    remaining: VecDeque<(GaugeCommand, i32)>,
    contributions: Vec<GaugeChange>,
}

impl GaugeChange {
    pub fn events(self) -> Vec<BattleEvent> {
        let changed = self.enabled_before != self.enabled_after
            || self.applied_delta != 0
            || self.progress_raw_delta != 0
            || self.overflow != 0;
        changed
            .then_some(BattleEvent::GaugeChanged(GaugeChangeEvent {
                origin: self.origin,
                key: self.key,
                source_uid: self.source_uid,
                source_skill_id: self.source_skill_id,
                config_effect: self.config_effect,
                progress_raw_delta: self.progress_raw_delta,
                kind: self.kind,
                before: self.before,
                requested_delta: self.requested_delta,
                applied_delta: self.applied_delta,
                after: self.after,
                overflow: self.overflow,
                before_max: self.before_max,
                after_max: self.after_max,
                enabled_before: self.enabled_before,
                enabled_after: self.enabled_after,
            }))
            .into_iter()
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GaugeCommandError {
    InvalidCommand,
    MissingGauge(GaugeKey),
}

#[derive(Debug, Clone, Default)]
/// Owns scoped gauge values, maxima, accumulators, and committed changes.
/// Mechanic rules decide what feeds a gauge and what reacts to its changes.
pub struct GaugeManager {
    states: HashMap<GaugeKey, GaugeState>,
    raw_values: HashMap<GaugeKey, i32>,
    base_max: HashMap<GaugeKey, i32>,
    opening_max: HashMap<GaugeKey, i32>,
    opening_setup: bool,
    accumulators: HashMap<GaugeKey, i32>,
    positive_gains: HashMap<GaugeKey, i32>,
    positive_raw_gains: HashMap<GaugeKey, i32>,
    consumed_raw_progress: HashMap<(GaugeKey, i64, i32), i32>,
    pending_raw_batches: HashMap<GaugeBatchId, PendingRawGaugeBatch>,
    positive_gain_round_start: HashMap<GaugeKey, i32>,
    positive_threshold_crossings_paid_this_round: HashMap<(GaugeKey, i64, i32), i32>,
}

impl GaugeManager {
    pub fn get(&self, key: GaugeKey) -> Option<GaugeState> {
        self.states.get(&key).copied()
    }

    pub fn base_max(&self, key: GaugeKey) -> Option<i32> {
        self.base_max.get(&key).copied()
    }

    pub fn accumulation_max(&self, key: GaugeKey) -> Option<i32> {
        if self.opening_setup
            && let Some(max) = self.opening_max.get(&key)
        {
            return Some(*max);
        }
        self.states.get(&key)?.max
    }

    pub fn raw_value(&self, key: GaugeKey) -> Option<i32> {
        self.states.get(&key)?;
        self.raw_values.get(&key).copied()
    }

    pub fn plan_raw_contributions(&self, key: GaugeKey, raw_amounts: &[i32]) -> Option<Vec<i32>> {
        if raw_amounts.is_empty() || raw_amounts.iter().any(|amount| *amount <= 0) {
            return None;
        }
        let state = self.states.get(&key).copied()?;
        let raw_before = self
            .raw_values
            .get(&key)
            .copied()
            .unwrap_or_else(|| state.current.saturating_mul(1000));
        let raw_total = raw_amounts
            .iter()
            .fold(0_i32, |total, amount| total.saturating_add(*amount));
        let raw_after = raw_before
            .saturating_add(raw_total)
            .min(state.max.map_or(i32::MAX, |max| max.saturating_mul(1000)));
        let available = state
            .max
            .map_or(i32::MAX, |max| max.saturating_sub(state.current))
            .max(0);
        let total_delta = (raw_after / 1000)
            .saturating_sub(state.current)
            .min(available);

        let mut deltas = vec![0; raw_amounts.len()];
        let mut remaining = total_delta;
        for (delta, raw_amount) in deltas.iter_mut().zip(raw_amounts) {
            let base = (raw_amount / 1000).min(remaining);
            *delta = base;
            remaining -= base;
        }
        let mut fractional = raw_amounts
            .iter()
            .enumerate()
            .map(|(index, amount)| (index, amount % 1000))
            .collect::<Vec<_>>();
        fractional.sort_by_key(|(index, remainder)| (std::cmp::Reverse(*remainder), *index));
        for (index, _) in fractional.into_iter().take(remaining as usize) {
            deltas[index] += 1;
        }
        Some(deltas)
    }

    pub fn accumulated_value(
        &self,
        key: GaugeKey,
        listener_uid: i64,
        listener_opcode: i32,
    ) -> Option<i32> {
        Some(self.accumulated_raw_value(key, listener_uid, listener_opcode)? / 1000)
    }

    pub fn accumulated_raw_value(
        &self,
        key: GaugeKey,
        listener_uid: i64,
        listener_opcode: i32,
    ) -> Option<i32> {
        self.states.get(&key)?;
        let gained = self
            .positive_raw_gains
            .get(&key)
            .copied()
            .unwrap_or_default();
        let consumed = self
            .consumed_raw_progress
            .get(&(key, listener_uid, listener_opcode))
            .copied()
            .unwrap_or_default();
        Some(gained.saturating_sub(consumed).max(0))
    }

    pub fn preview_accumulated_change(
        &self,
        key: GaugeKey,
        listener_uid: i64,
        listener_opcode: i32,
        raw_delta: i32,
    ) -> Option<i32> {
        self.states.get(&key)?;
        let gained = self
            .positive_raw_gains
            .get(&key)
            .copied()
            .unwrap_or_default();
        let consumed = self
            .consumed_raw_progress
            .get(&(key, listener_uid, listener_opcode))
            .copied()
            .unwrap_or_default();
        Some(
            gained
                .saturating_sub(consumed)
                .saturating_add(raw_delta.max(0))
                .max(0)
                / 1000,
        )
    }

    pub fn preview_accumulated_value(
        &self,
        key: GaugeKey,
        amount: i32,
        threshold: i32,
    ) -> Option<i32> {
        if amount <= 0 || threshold <= 0 {
            return None;
        }
        let state = self.get(key)?;
        let capacity = state.max.map_or(i32::MAX, |max| max - state.current).max(0);
        let accumulated = self
            .accumulators
            .get(&key)
            .copied()
            .unwrap_or_default()
            .saturating_add(amount);
        Some((accumulated / threshold).min(capacity))
    }

    pub fn begin_opening_setup(&mut self) {
        self.opening_setup = true;
        self.opening_max.clear();
    }

    pub fn finish_opening_setup(&mut self) {
        self.opening_setup = false;
        self.opening_max.clear();
    }

    pub fn begin_combat_round(&mut self) {
        self.positive_gain_round_start = self.positive_gains.clone();
        self.positive_threshold_crossings_paid_this_round.clear();
    }

    pub fn positive_gain_since_round_start(&self, key: GaugeKey) -> i32 {
        self.positive_gains
            .get(&key)
            .copied()
            .unwrap_or_default()
            .saturating_sub(
                self.positive_gain_round_start
                    .get(&key)
                    .copied()
                    .unwrap_or_default(),
            )
    }

    pub fn settle_positive_threshold(
        &mut self,
        key: GaugeKey,
        listener_uid: i64,
        listener_opcode: i32,
        threshold: i32,
        amount: i32,
    ) -> i32 {
        if listener_uid == 0 || threshold <= 0 || amount <= 0 || !self.states.contains_key(&key) {
            return 0;
        }
        let delta =
            self.preview_positive_threshold(key, listener_uid, listener_opcode, threshold, amount);
        if delta > 0 {
            *self
                .positive_threshold_crossings_paid_this_round
                .entry((key, listener_uid, listener_opcode))
                .or_default() += delta / amount;
        }
        delta
    }

    pub fn preview_positive_threshold(
        &self,
        key: GaugeKey,
        listener_uid: i64,
        listener_opcode: i32,
        threshold: i32,
        amount: i32,
    ) -> i32 {
        if listener_uid == 0 || threshold <= 0 || amount <= 0 || !self.states.contains_key(&key) {
            return 0;
        }
        let total = self.positive_gains.get(&key).copied().unwrap_or_default();
        let round_start = self
            .positive_gain_round_start
            .get(&key)
            .copied()
            .unwrap_or_default();
        let eligible_crossings = total.saturating_sub(round_start).max(0) / threshold;
        let paid_crossings = self
            .positive_threshold_crossings_paid_this_round
            .get(&(key, listener_uid, listener_opcode))
            .copied()
            .unwrap_or_default();
        eligible_crossings.saturating_sub(paid_crossings) * amount
    }

    pub(crate) fn execute_command(
        &mut self,
        command: GaugeCommand,
    ) -> Result<GaugeChange, GaugeCommandError> {
        if self
            .pending_raw_batches
            .keys()
            .any(|batch| batch.key == command.key)
        {
            return Err(GaugeCommandError::InvalidCommand);
        }
        match command.operation {
            GaugeOperation::Enable { max } => {
                if max.is_some_and(|value| value < 0) {
                    return Err(GaugeCommandError::InvalidCommand);
                }
                let existing = self.states.get(&command.key).copied();
                if existing.is_none()
                    && let Some(max) = max
                {
                    self.base_max.insert(command.key, max);
                    if self.opening_setup {
                        self.opening_max.insert(command.key, max);
                    }
                }
                let before = existing.unwrap_or_default();
                let after = *self
                    .states
                    .entry(command.key)
                    .or_insert(GaugeState { current: 0, max });
                self.raw_values
                    .entry(command.key)
                    .or_insert(after.current.saturating_mul(1000));
                Ok(change(
                    command,
                    GaugeChangeKind::Enabled,
                    before,
                    after,
                    0,
                    0,
                    (existing.is_some(), true),
                ))
            }
            GaugeOperation::ChangeValue { delta } => self.change_value(command, delta, false),
            GaugeOperation::AccumulateRawValue { amount, stream } => {
                if amount <= 0 || stream <= 0 {
                    return Err(GaugeCommandError::InvalidCommand);
                }
                let state = self
                    .states
                    .get(&command.key)
                    .copied()
                    .ok_or(GaugeCommandError::MissingGauge(command.key))?;
                let capacity = state.max.map_or(i32::MAX, |max| max - state.current).max(0);
                if capacity == 0 {
                    return self.change_value(command, 0, true);
                }
                let raw_before = self
                    .raw_values
                    .get(&command.key)
                    .copied()
                    .unwrap_or_else(|| state.current.saturating_mul(1000));
                let raw_after = raw_before
                    .saturating_add(amount)
                    .min(state.max.map_or(i32::MAX, |max| max.saturating_mul(1000)));
                let visible_after = raw_after / 1000;
                let delta = visible_after.saturating_sub(state.current).min(capacity);
                let command = command
                    .with_raw_delta(amount)
                    .with_progress_raw_delta(amount);
                self.change_value(command, delta, true)
            }
            GaugeOperation::AccumulateValue { amount, threshold } => {
                if amount <= 0 || threshold <= 0 {
                    return Err(GaugeCommandError::InvalidCommand);
                }
                let state = self
                    .states
                    .get(&command.key)
                    .copied()
                    .ok_or(GaugeCommandError::MissingGauge(command.key))?;
                let capacity = state.max.map_or(i32::MAX, |max| max - state.current).max(0);
                if capacity == 0 {
                    return self.change_value(command, 0, true);
                }
                let accumulator = self.accumulators.entry(command.key).or_default();
                let accumulator_before = *accumulator;
                *accumulator = accumulator.saturating_add(amount);
                let delta = (*accumulator / threshold).min(capacity);
                *accumulator -= delta * threshold;
                if crate::engine::diagnostics::enabled(crate::engine::diagnostics::TraceArea::Gauge)
                {
                    eprintln!(
                        "gauge accumulation key={:?} origin={:?} source={} skill={} amount={} threshold={} remainder={}->{} delta={}",
                        command.key,
                        command.origin,
                        command.source_uid,
                        command.source_skill_id,
                        amount,
                        threshold,
                        accumulator_before,
                        *accumulator,
                        delta,
                    );
                }
                self.change_value(command, delta, true)
            }
            GaugeOperation::ChangeMax { delta } => {
                if delta == 0 {
                    return Err(GaugeCommandError::InvalidCommand);
                }
                let state = self
                    .states
                    .get_mut(&command.key)
                    .ok_or(GaugeCommandError::MissingGauge(command.key))?;
                let before = *state;
                let Some(max) = state.max else {
                    return Err(GaugeCommandError::InvalidCommand);
                };
                state.max = Some(max.saturating_add(delta).max(0));
                state.current = state.current.min(state.max.unwrap_or_default());
                if let Some(raw) = self.raw_values.get_mut(&command.key) {
                    *raw = (*raw).min(state.max.unwrap_or_default().saturating_mul(1000));
                }
                Ok(change(
                    command,
                    GaugeChangeKind::Max,
                    before,
                    *state,
                    delta,
                    0,
                    (true, true),
                ))
            }
            GaugeOperation::SyncBaseMax { max } => {
                if max < 0 {
                    return Err(GaugeCommandError::InvalidCommand);
                }
                let old_base = self
                    .base_max
                    .get_mut(&command.key)
                    .ok_or(GaugeCommandError::MissingGauge(command.key))?;
                let delta = max.saturating_sub(*old_base);
                let state = self
                    .states
                    .get_mut(&command.key)
                    .ok_or(GaugeCommandError::MissingGauge(command.key))?;
                let before = *state;
                let Some(current_max) = state.max else {
                    return Err(GaugeCommandError::InvalidCommand);
                };
                *old_base = max;
                state.max = Some(current_max.saturating_add(delta).max(0));
                state.current = state.current.min(state.max.unwrap_or_default());
                if let Some(raw) = self.raw_values.get_mut(&command.key) {
                    *raw = (*raw).min(state.max.unwrap_or_default().saturating_mul(1000));
                }
                Ok(change(
                    command,
                    GaugeChangeKind::Max,
                    before,
                    *state,
                    delta,
                    0,
                    (true, true),
                ))
            }
            GaugeOperation::ConsumeAccumulated {
                listener_uid,
                listener_opcode,
                amount,
            } => {
                if listener_uid == 0 || listener_opcode <= 0 || amount <= 0 {
                    return Err(GaugeCommandError::InvalidCommand);
                }
                let before = self
                    .accumulated_value(command.key, listener_uid, listener_opcode)
                    .ok_or(GaugeCommandError::MissingGauge(command.key))?;
                let applied = amount.min(before);
                *self
                    .consumed_raw_progress
                    .entry((command.key, listener_uid, listener_opcode))
                    .or_default() += applied.saturating_mul(1000);
                let after = before - applied;
                Ok(change(
                    command,
                    GaugeChangeKind::Accumulated,
                    GaugeState {
                        current: before,
                        max: None,
                    },
                    GaugeState {
                        current: after,
                        max: None,
                    },
                    -amount,
                    0,
                    (true, true),
                ))
            }
            GaugeOperation::AccumulateProgress { raw_amount } => {
                if raw_amount <= 0 {
                    return Err(GaugeCommandError::InvalidCommand);
                }
                self.states
                    .get(&command.key)
                    .ok_or(GaugeCommandError::MissingGauge(command.key))?;
                let raw = self.positive_raw_gains.entry(command.key).or_default();
                let before = *raw / 1000;
                *raw = raw.saturating_add(raw_amount);
                let after = *raw / 1000;
                Ok(change(
                    command,
                    GaugeChangeKind::Accumulated,
                    GaugeState {
                        current: before,
                        max: None,
                    },
                    GaugeState {
                        current: after,
                        max: None,
                    },
                    raw_amount / 1000,
                    0,
                    (true, true),
                ))
            }
            GaugeOperation::Snapshot => {
                let state = self
                    .states
                    .get(&command.key)
                    .copied()
                    .ok_or(GaugeCommandError::MissingGauge(command.key))?;
                Ok(change(
                    command,
                    GaugeChangeKind::Snapshot,
                    state,
                    state,
                    0,
                    0,
                    (true, true),
                ))
            }
            GaugeOperation::Disable => {
                self.base_max.remove(&command.key);
                self.opening_max.remove(&command.key);
                self.accumulators.remove(&command.key);
                self.raw_values.remove(&command.key);
                self.positive_gains.remove(&command.key);
                self.positive_raw_gains.remove(&command.key);
                self.consumed_raw_progress
                    .retain(|(key, _, _), _| *key != command.key);
                self.positive_gain_round_start.remove(&command.key);
                self.positive_threshold_crossings_paid_this_round
                    .retain(|(key, _, _), _| *key != command.key);
                let before = self
                    .states
                    .remove(&command.key)
                    .ok_or(GaugeCommandError::MissingGauge(command.key))?;
                Ok(change(
                    command,
                    GaugeChangeKind::Disabled,
                    before,
                    GaugeState::default(),
                    0,
                    0,
                    (true, false),
                ))
            }
        }
    }

    pub(crate) fn start_raw_batch(
        &mut self,
        id: GaugeBatchId,
        commands: Vec<GaugeCommand>,
    ) -> Result<GaugeChange, GaugeCommandError> {
        if self.pending_raw_batches.contains_key(&id)
            || self
                .pending_raw_batches
                .keys()
                .any(|pending| pending.key == id.key)
        {
            return Err(GaugeCommandError::InvalidCommand);
        }
        let Some(first_command) = commands.first().copied() else {
            return Err(GaugeCommandError::InvalidCommand);
        };
        if id.origin != first_command.origin
            || id.key != first_command.key
            || id.source_uid != first_command.source_uid
            || id.discriminator == 0
        {
            return Err(GaugeCommandError::InvalidCommand);
        }
        let raw_amounts = commands
            .iter()
            .map(|command| match command.operation {
                GaugeOperation::AccumulateRawValue { amount, .. }
                    if command.key == first_command.key
                        && command.origin == first_command.origin
                        && command.source_uid == first_command.source_uid
                        && command.source_skill_id == first_command.source_skill_id
                        && command.config_effect == first_command.config_effect
                        && amount > 0 =>
                {
                    Ok(amount)
                }
                _ => Err(GaugeCommandError::InvalidCommand),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let deltas = self
            .plan_raw_contributions(first_command.key, &raw_amounts)
            .ok_or(GaugeCommandError::InvalidCommand)?;
        self.pending_raw_batches.insert(
            id,
            PendingRawGaugeBatch {
                remaining: commands
                    .into_iter()
                    .zip(raw_amounts)
                    .zip(deltas)
                    .map(|((command, raw_amount), value_delta)| {
                        (command.with_raw_delta(raw_amount), value_delta)
                    })
                    .collect(),
                contributions: Vec::new(),
            },
        );
        self.continue_raw_batch(id)
    }

    pub(crate) fn continue_raw_batch(
        &mut self,
        id: GaugeBatchId,
    ) -> Result<GaugeChange, GaugeCommandError> {
        let (command, value_delta) = self
            .pending_raw_batches
            .get(&id)
            .and_then(|batch| batch.remaining.front().copied())
            .ok_or(GaugeCommandError::InvalidCommand)?;
        let change = self.change_value(command, value_delta, true)?;
        let batch = self
            .pending_raw_batches
            .get_mut(&id)
            .ok_or(GaugeCommandError::InvalidCommand)?;
        batch.remaining.pop_front();
        batch.contributions.push(change);
        Ok(change)
    }

    pub(crate) fn commit_raw_batch(
        &mut self,
        id: GaugeBatchId,
    ) -> Result<GaugeBatchChanges, GaugeCommandError> {
        if self
            .pending_raw_batches
            .get(&id)
            .is_none_or(|batch| !batch.remaining.is_empty())
        {
            return Err(GaugeCommandError::InvalidCommand);
        }
        let batch = self
            .pending_raw_batches
            .remove(&id)
            .ok_or(GaugeCommandError::InvalidCommand)?;
        let first = batch
            .contributions
            .first()
            .copied()
            .ok_or(GaugeCommandError::InvalidCommand)?;
        let last = batch.contributions.last().copied().unwrap_or(first);
        let applied_delta = batch.contributions.iter().fold(0_i32, |total, change| {
            total.saturating_add(change.applied_delta)
        });
        let combined = GaugeChange {
            before: first.before,
            requested_delta: batch.contributions.iter().fold(0_i32, |total, change| {
                total.saturating_add(change.requested_delta)
            }),
            applied_delta,
            after: first.before.saturating_add(applied_delta),
            overflow: batch
                .contributions
                .iter()
                .fold(0_i32, |total, change| total.saturating_add(change.overflow)),
            progress_raw_delta: batch.contributions.iter().fold(0_i32, |total, change| {
                total.saturating_add(change.progress_raw_delta)
            }),
            after_max: last.after_max,
            enabled_after: last.enabled_after,
            ..first
        };
        Ok(GaugeBatchChanges { combined })
    }

    fn change_value(
        &mut self,
        mut command: GaugeCommand,
        delta: i32,
        allow_zero: bool,
    ) -> Result<GaugeChange, GaugeCommandError> {
        if delta == 0 && !allow_zero {
            return Err(GaugeCommandError::InvalidCommand);
        }
        let state = self
            .states
            .get_mut(&command.key)
            .ok_or(GaugeCommandError::MissingGauge(command.key))?;
        let before = *state;
        let requested = state.current.saturating_add(delta);
        state.current = match state.max {
            Some(max) => requested.clamp(0, max),
            None => requested.max(0),
        };
        let overflow = if delta > 0 {
            (delta - (state.current - before.current)).max(0)
        } else {
            0
        };
        let applied = state.current - before.current;
        let after = *state;
        let raw_delta = command
            .raw_delta
            .unwrap_or_else(|| applied.saturating_mul(1000));
        let raw = self
            .raw_values
            .entry(command.key)
            .or_insert(before.current.saturating_mul(1000));
        let requested_raw = raw.saturating_add(raw_delta);
        *raw = match after.max {
            Some(max) => requested_raw.clamp(0, max.saturating_mul(1000)),
            None => requested_raw.max(0),
        };
        if applied > 0 {
            *self.positive_gains.entry(command.key).or_default() += applied;
        }
        let progress_raw_delta = command.progress_raw_delta.unwrap_or(raw_delta);
        let has_capacity = before.max.is_none_or(|max| before.current < max);
        let eligible_progress =
            if progress_raw_delta > 0 && (applied > 0 || (allow_zero && has_capacity)) {
                *self.positive_raw_gains.entry(command.key).or_default() += progress_raw_delta;
                progress_raw_delta
            } else {
                0
            };
        command.progress_raw_delta = Some(eligible_progress);
        Ok(change(
            command,
            GaugeChangeKind::Value,
            before,
            after,
            delta,
            overflow,
            (true, true),
        ))
    }
}

fn change(
    command: GaugeCommand,
    kind: GaugeChangeKind,
    before: GaugeState,
    after: GaugeState,
    requested_delta: i32,
    overflow: i32,
    enabled: (bool, bool),
) -> GaugeChange {
    let applied_delta = match kind {
        GaugeChangeKind::Max => after.max.unwrap_or_default() - before.max.unwrap_or_default(),
        _ => after.current - before.current,
    };
    GaugeChange {
        origin: command.origin,
        key: command.key,
        source_uid: command.source_uid,
        source_skill_id: command.source_skill_id,
        config_effect: command.config_effect,
        progress_raw_delta: command.progress_raw_delta.unwrap_or_default(),
        kind,
        before: before.current,
        requested_delta,
        applied_delta,
        after: after.current,
        overflow,
        before_max: before.max,
        after_max: after.max,
        enabled_before: enabled.0,
        enabled_after: enabled.1,
    }
}

#[cfg(test)]
mod tests;
