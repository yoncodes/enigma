use std::collections::HashMap;

use sonettobuf::{Fight, FightEntityInfo};

use crate::engine::{
    event::payload::{BattleEvent, ExPointChangeEvent},
    skill::rule::CommandOrigin,
};

use super::entities;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExPointKind {
    Common,
    Faith,
    Synchronization,
    Adrenaline,
    DevicePower,
    NoExPoint,
}

impl ExPointKind {
    pub fn from_wire(value: i32) -> Self {
        match value {
            0 => Self::Common,
            1 => Self::Faith,
            2 => Self::Synchronization,
            3 => Self::Adrenaline,
            4 => Self::DevicePower,
            999 => Self::NoExPoint,
            _ => Self::Common,
        }
    }

    pub fn as_wire(self) -> i32 {
        match self {
            Self::Common => 0,
            Self::Faith => 1,
            Self::Synchronization => 2,
            Self::Adrenaline => 3,
            Self::DevicePower => 4,
            Self::NoExPoint => 999,
        }
    }

    pub fn default_max(self) -> i32 {
        match self {
            Self::Common => 5,
            Self::Faith
            | Self::Synchronization
            | Self::Adrenaline
            | Self::DevicePower
            | Self::NoExPoint => 0,
        }
    }

    pub fn can_bank_overflow(
        self,
        event_kind: Self,
        event_target_uid: i64,
        owner_uid: i64,
    ) -> bool {
        matches!(
            (self, event_kind),
            (Self::Common, Self::Common) | (Self::Faith, Self::Faith)
        ) && event_target_uid != 0
            && event_target_uid == owner_uid
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExPointState {
    pub current: i32,
    pub max: i32,
    pub kind: i32,
    pub recent_decrement: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SynchronizationDefinition {
    pub skills: [i32; 3],
    pub action_count: i32,
    pub threshold: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SynchronizationProgress {
    pub completed_actions: i32,
    pub target_uid: i64,
    pub total_damage: i32,
}

impl SynchronizationDefinition {
    pub fn new(skills: [i32; 3], action_count: i32, threshold: i32) -> Option<Self> {
        (skills.iter().all(|skill_id| *skill_id > 0) && action_count > 0 && threshold > 0)
            .then_some(Self {
                skills,
                action_count,
                threshold,
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExPointApplyResult {
    pub source_uid: i64,
    pub target_uid: i64,
    pub kind: ExPointKind,
    pub before: i32,
    pub requested_delta: i32,
    pub applied_delta: i32,
    pub after: i32,
    pub overflow: i32,
    pub cap: i32,
    pub effect_type: i32,
    pub config_effect: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExPointChange {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub delta: i32,
    pub config_effect: i32,
    pub effect_type: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExPointSet {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub value: i32,
    pub config_effect: i32,
    pub effect_type: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExPointMaxChange {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub delta: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExPointConfigureSynchronization {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub definition: SynchronizationDefinition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExPointRecordSynchronizationAction {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub action_target_uid: i64,
    pub damage: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExPointMaxApplyResult {
    pub target_uid: i64,
    pub before: i32,
    pub requested_delta: i32,
    pub applied_delta: i32,
    pub after: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExPointCommand {
    Change(ExPointChange),
    Spend(ExPointChange),
    Set(ExPointSet),
    ChangeMax(ExPointMaxChange),
    ConfigureSynchronization(ExPointConfigureSynchronization),
    RecordSynchronizationAction(ExPointRecordSynchronizationAction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExPointChanges {
    Value {
        origin: CommandOrigin,
        change: ExPointApplyResult,
    },
    Max {
        origin: CommandOrigin,
        change: ExPointMaxApplyResult,
    },
    SynchronizationConfigured {
        origin: CommandOrigin,
        target_uid: i64,
        definition: SynchronizationDefinition,
    },
    SynchronizationAdvanced {
        origin: CommandOrigin,
        target_uid: i64,
        progress: SynchronizationProgress,
    },
}

impl ExPointChanges {
    pub fn events(self) -> Vec<BattleEvent> {
        let Self::Value { origin, change } = self else {
            return Vec::new();
        };
        let event = ExPointChangeEvent {
            origin,
            source_uid: change.source_uid,
            target_uid: change.target_uid,
            kind: change.kind,
            before: change.before,
            requested_delta: change.requested_delta,
            applied_delta: change.applied_delta,
            after: change.after,
            overflow: change.overflow,
        };
        let mut events = Vec::with_capacity(2);
        if change.applied_delta != 0 {
            events.push(BattleEvent::ExPointChanged(event));
        }
        if change.overflow > 0 {
            events.push(BattleEvent::ExPointOverflow(event));
        }
        events
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExPointCommandError {
    InvalidCommand,
    MissingTarget(i64),
}

#[derive(Debug, Clone, Default)]
pub struct ExPointManager {
    states: HashMap<i64, ExPointState>,
    synchronization: HashMap<i64, SynchronizationDefinition>,
    synchronization_progress: HashMap<i64, SynchronizationProgress>,
}

impl ExPointManager {
    pub(crate) fn execute_command(
        &mut self,
        command: ExPointCommand,
        gain_allowed: bool,
        reduction_allowed: bool,
    ) -> Result<ExPointChanges, ExPointCommandError> {
        if let ExPointCommand::ConfigureSynchronization(configure) = command {
            let state = self
                .states
                .get(&configure.target_uid)
                .ok_or(ExPointCommandError::MissingTarget(configure.target_uid))?;
            if ExPointKind::from_wire(state.kind) != ExPointKind::Synchronization {
                return Err(ExPointCommandError::InvalidCommand);
            }
            self.synchronization
                .insert(configure.target_uid, configure.definition);
            self.synchronization_progress
                .insert(configure.target_uid, SynchronizationProgress::default());
            return Ok(ExPointChanges::SynchronizationConfigured {
                origin: configure.origin,
                target_uid: configure.target_uid,
                definition: configure.definition,
            });
        }
        if let ExPointCommand::RecordSynchronizationAction(record) = command {
            let definition = self
                .synchronization
                .get(&record.target_uid)
                .ok_or(ExPointCommandError::InvalidCommand)?;
            if record.action_target_uid == 0 || record.damage < 0 {
                return Err(ExPointCommandError::InvalidCommand);
            }
            let progress = self
                .synchronization_progress
                .entry(record.target_uid)
                .or_default();
            if progress.completed_actions >= definition.action_count {
                return Err(ExPointCommandError::InvalidCommand);
            }
            progress.completed_actions += 1;
            progress.target_uid = record.action_target_uid;
            progress.total_damage = progress.total_damage.saturating_add(record.damage);
            return Ok(ExPointChanges::SynchronizationAdvanced {
                origin: record.origin,
                target_uid: record.target_uid,
                progress: *progress,
            });
        }
        if let ExPointCommand::ChangeMax(change) = command {
            if change.target_uid == 0 || change.delta == 0 {
                return Err(ExPointCommandError::InvalidCommand);
            }
            let state = self
                .states
                .get_mut(&change.target_uid)
                .ok_or(ExPointCommandError::MissingTarget(change.target_uid))?;
            let before = Self::cap_for_state(*state);
            let after = (before + change.delta).max(0);
            state.max = after;
            state.current = Self::clamp_value(state.current, after);
            return Ok(ExPointChanges::Max {
                origin: change.origin,
                change: ExPointMaxApplyResult {
                    target_uid: change.target_uid,
                    before,
                    requested_delta: change.delta,
                    applied_delta: after - before,
                    after,
                },
            });
        }
        let (origin, source_uid, target_uid, value, config_effect, effect_type, set, spend) =
            match command {
                ExPointCommand::Change(value) => (
                    value.origin,
                    value.source_uid,
                    value.target_uid,
                    value.delta,
                    value.config_effect,
                    value.effect_type,
                    false,
                    false,
                ),
                ExPointCommand::Spend(value) => (
                    value.origin,
                    value.source_uid,
                    value.target_uid,
                    value.delta,
                    value.config_effect,
                    value.effect_type,
                    false,
                    true,
                ),
                ExPointCommand::Set(value) => (
                    value.origin,
                    value.source_uid,
                    value.target_uid,
                    value.value,
                    value.config_effect,
                    value.effect_type,
                    true,
                    false,
                ),
                ExPointCommand::ChangeMax(_) => unreachable!(),
                ExPointCommand::ConfigureSynchronization(_) => unreachable!(),
                ExPointCommand::RecordSynchronizationAction(_) => unreachable!(),
            };
        if target_uid == 0 || (!set && value == 0) || (set && value < 0) {
            return Err(ExPointCommandError::InvalidCommand);
        }
        if !self.states.contains_key(&target_uid) {
            return Err(ExPointCommandError::MissingTarget(target_uid));
        }
        let delta = if set {
            value - self.get(target_uid)
        } else {
            value
        };
        if (delta > 0 && !gain_allowed) || (delta < 0 && !spend && !reduction_allowed) {
            let state = self.states[&target_uid];
            return Ok(ExPointChanges::Value {
                origin,
                change: ExPointApplyResult {
                    source_uid,
                    target_uid,
                    kind: ExPointKind::from_wire(state.kind),
                    before: state.current,
                    requested_delta: delta,
                    applied_delta: 0,
                    after: state.current,
                    overflow: 0,
                    cap: Self::cap_for_state(state),
                    effect_type,
                    config_effect,
                },
            });
        }
        Ok(ExPointChanges::Value {
            origin,
            change: self.apply(source_uid, target_uid, delta, config_effect, effect_type),
        })
    }

    pub fn seed(&mut self, fight: &Fight) {
        self.states.clear();
        self.synchronization.clear();
        self.synchronization_progress.clear();
        for entity in entities(fight).chain(
            fight
                .attacker
                .iter()
                .chain(fight.defender.iter())
                .filter_map(|team| team.assist_boss.as_ref()),
        ) {
            self.register(entity);
        }
    }

    pub fn register(&mut self, entity: &FightEntityInfo) {
        let Some(uid) = entity.uid else { return };
        let kind = ExPointKind::from_wire(entity.ex_point_type.unwrap_or_default());
        let base_max = configured_max(entity).unwrap_or_else(|| kind.default_max());
        self.states.insert(
            uid,
            Self::normalize(ExPointState {
                current: entity.ex_point.unwrap_or_default(),
                max: base_max + entity.expoint_max_add.unwrap_or_default(),
                kind: kind.as_wire(),
                recent_decrement: 0,
            }),
        );
    }

    pub fn get(&self, uid: i64) -> i32 {
        self.states
            .get(&uid)
            .map(|state| state.current)
            .unwrap_or_default()
    }

    pub fn is_full(&self, uid: i64) -> bool {
        self.states.get(&uid).is_some_and(|state| {
            let cap = Self::cap_for_state(*state);
            cap > 0 && state.current >= cap
        })
    }

    pub fn kind(&self, uid: i64) -> i32 {
        self.states
            .get(&uid)
            .map(|state| state.kind)
            .unwrap_or_default()
    }

    pub fn gains_composition_ex_point(&self, uid: i64) -> bool {
        self.kind(uid) == ExPointKind::Common.as_wire()
    }

    pub fn synchronization_definition(&self, uid: i64) -> Option<SynchronizationDefinition> {
        self.synchronization.get(&uid).copied()
    }

    pub fn synchronization_progress(&self, uid: i64) -> Option<SynchronizationProgress> {
        self.synchronization_progress.get(&uid).copied()
    }

    pub fn add(
        &mut self,
        source_uid: i64,
        target_uid: i64,
        delta: i32,
        config_effect: i32,
    ) -> ExPointApplyResult {
        self.apply(source_uid, target_uid, delta, config_effect, 0)
    }

    pub fn set(
        &mut self,
        source_uid: i64,
        target_uid: i64,
        value: i32,
        config_effect: i32,
    ) -> ExPointApplyResult {
        let before = self.get(target_uid);
        self.apply(source_uid, target_uid, value - before, config_effect, 0)
    }

    pub fn add_max(&mut self, target_uid: i64, delta: i32) {
        let state = self
            .states
            .entry(target_uid)
            .or_insert_with(|| Self::normalize(ExPointState::default()));
        state.max = (Self::cap_for_state(*state) + delta).max(0);
        state.current = Self::clamp_value(state.current, state.max);
    }

    pub fn sync_entity(&self, entity: &mut FightEntityInfo) {
        let Some(uid) = entity.uid else { return };
        let state = self.states.get(&uid).copied().unwrap_or_default();
        entity.ex_point = Some(state.current);
        let base_max = configured_max(entity)
            .unwrap_or_else(|| ExPointKind::from_wire(state.kind).default_max());
        entity.expoint_max_add = Some((self.cap_for(state) - base_max).max(0));
        entity.ex_point_type = Some(state.kind);
    }

    fn apply(
        &mut self,
        source_uid: i64,
        target_uid: i64,
        delta: i32,
        config_effect: i32,
        effect_type: i32,
    ) -> ExPointApplyResult {
        let state = self
            .states
            .entry(target_uid)
            .or_insert_with(|| Self::normalize(ExPointState::default()));
        let kind = ExPointKind::from_wire(state.kind);
        let before = state.current;
        let cap = Self::cap_for_state(*state);
        let raw_after = before + delta;
        let after = Self::clamp_value(raw_after, cap);
        let applied_delta = after - before;
        let overflow = if delta > 0 {
            (delta - applied_delta).max(0)
        } else {
            0
        };
        state.current = after;
        state.recent_decrement = if applied_delta < 0 { -applied_delta } else { 0 };

        ExPointApplyResult {
            source_uid,
            target_uid,
            kind,
            before,
            requested_delta: delta,
            applied_delta,
            after,
            overflow,
            cap,
            effect_type,
            config_effect,
        }
    }

    fn normalize(state: ExPointState) -> ExPointState {
        let cap = Self::cap_for_state(state);
        ExPointState {
            current: Self::clamp_value(state.current, cap),
            max: cap,
            ..state
        }
    }

    fn cap_for(&self, state: ExPointState) -> i32 {
        Self::cap_for_state(state)
    }

    fn cap_for_state(state: ExPointState) -> i32 {
        if state.max > 0 {
            state.max
        } else {
            ExPointKind::from_wire(state.kind).default_max()
        }
    }

    fn clamp_value(value: i32, cap: i32) -> i32 {
        if cap > 0 {
            value.clamp(0, cap)
        } else {
            value.max(0)
        }
    }
}

fn configured_max(entity: &FightEntityInfo) -> Option<i32> {
    let db = config::try_get()?;
    let hero_id = entity.model_id?;
    let rank = crate::engine::entity::stats::rank_from_level(hero_id, entity.level.unwrap_or(1));
    let spec = if rank > 2 {
        db.character_rank_replace
            .get(hero_id)
            .map(|row| row.unique_skill_point.as_str())
    } else {
        None
    }
    .or_else(|| {
        db.character
            .get(hero_id)
            .map(|row| row.unique_skill_point.as_str())
    })?;

    spec.split('#').nth(1)?.trim().parse().ok()
}

#[cfg(test)]
mod tests;
