use std::collections::{BTreeMap, HashSet};

use crate::engine::skill::rule::CommandOrigin;

mod execute;
mod seed;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConduitSkill {
    pub skill_id: i32,
    pub cost_type: i32,
    pub cost_value: i32,
    pub is_stopped: bool,
}

impl ConduitSkill {
    pub(crate) fn cost_after_reduction(self, reduction: i32) -> i32 {
        reduced_cost(self.cost_value, reduction)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConduitDevice {
    pub uid: i64,
    pub selected_group: i32,
    pub skill_groups: Vec<Vec<ConduitSkill>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConduitPower {
    pub id: i32,
    pub value: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConduitArea {
    pub team: i32,
    pub devices: Vec<ConduitDevice>,
    pub powers: Vec<ConduitPower>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConduitCommand {
    Initialize {
        team: i32,
    },
    SelectGroup {
        source_uid: i64,
        group: i32,
    },
    SetSkillGroup {
        origin: CommandOrigin,
        source_uid: i64,
        group: i32,
    },
    BeginSkill {
        source_uid: i64,
        skill_id: i32,
        cost_reduction: i32,
    },
    CommitSkillCost {
        source_uid: i64,
        skill_id: i32,
    },
    FinishSkill {
        source_uid: i64,
        skill_id: i32,
    },
    CompleteActivation {
        source_uid: i64,
        skill_id: i32,
    },
    SetRunning {
        source_uid: i64,
        running: bool,
    },
    ChangePower(ConduitPowerChange),
    ChangeCounter(ConduitCounterChange),
    ClearPowers {
        origin: CommandOrigin,
        source_uid: i64,
        team: i32,
        skill_id: i32,
        power_ids: [i32; 2],
    },
    ResetPowers {
        team: i32,
    },
    StopSkill {
        origin: CommandOrigin,
        source_uid: i64,
        team: i32,
        skill_id: i32,
    },
    RestartDevice {
        source_uid: i64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConduitPowerChangeKind {
    Standard,
    Interval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConduitPowerChange {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub team: i32,
    pub power_id: i32,
    pub delta: i32,
    pub kind: ConduitPowerChangeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConduitCounterKind {
    EnergyAccumulation,
    Activation,
}

impl ConduitCounterKind {
    pub fn from_config(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::EnergyAccumulation),
            2 => Some(Self::Activation),
            _ => None,
        }
    }

    pub fn wire_id(self) -> i32 {
        match self {
            Self::EnergyAccumulation => 62,
            Self::Activation => 63,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConduitCounterChange {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub team: i32,
    pub kind: ConduitCounterKind,
    pub delta: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConduitChange {
    Initialized(ConduitArea),
    GroupSelected {
        source_uid: i64,
        team: i32,
        group: i32,
    },
    SkillGroupChanged {
        origin: CommandOrigin,
        source_uid: i64,
        team: i32,
        group: i32,
    },
    SkillBegan {
        source_uid: i64,
        team: i32,
        skill_id: i32,
        power_id: i32,
        activation_cost: i32,
        spent: i32,
    },
    SkillCostCommitted {
        source_uid: i64,
        team: i32,
        skill_id: i32,
        power_id: i32,
        activation_cost: i32,
        consumed_this_round: i32,
    },
    SkillFinished {
        source_uid: i64,
        team: i32,
        skill_id: i32,
        uses_this_round: i32,
    },
    ActivationCompleted(crate::engine::event::payload::ConduitActivatedEvent),
    RunningChanged {
        source_uid: i64,
        running: bool,
    },
    PowerChanged {
        origin: CommandOrigin,
        source_uid: i64,
        team: i32,
        power_id: i32,
        requested_delta: i32,
        applied_delta: i32,
        after: i32,
        kind: ConduitPowerChangeKind,
    },
    CounterChanged {
        origin: CommandOrigin,
        source_uid: i64,
        team: i32,
        kind: ConduitCounterKind,
        requested_delta: i32,
        applied_delta: i32,
        after: i32,
    },
    PowersCleared {
        origin: CommandOrigin,
        source_uid: i64,
        team: i32,
        skill_id: i32,
        power_ids: [i32; 2],
        spent: i32,
    },
    PowersReset {
        team: i32,
    },
    SkillStopped {
        origin: CommandOrigin,
        source_uid: i64,
        team: i32,
        skill_id: i32,
    },
    DeviceRestarted {
        source_uid: i64,
        team: i32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConduitError {
    MissingDefinition(i32),
    InvalidSkill {
        device_id: i32,
        group: ConduitSkillGroup,
    },
    MissingArea(i32),
    AlreadyInitialized(i32),
    MissingDevice(i64),
    InvalidGroup {
        source_uid: i64,
        group: i32,
    },
    MissingSkill(i32),
    MissingActivation(i32),
    ActivationInProgress(i32),
    ActivationAlreadyCommitted(i32),
    ActivationNotCommitted(i32),
    StoppedSkill(i32),
    UnsupportedCostType(i32),
    InsufficientPower {
        power_id: i32,
        available: i32,
        required: i32,
    },
}

#[derive(Debug, Clone, Copy)]
struct PendingActivation {
    event: crate::engine::event::payload::ConduitActivatedEvent,
    cost_committed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConduitSkillGroup {
    Primary,
    Secondary,
    Unique,
}

#[derive(Debug, Clone, Default)]
pub struct ConduitManager {
    areas: BTreeMap<i32, ConduitArea>,
    initialization_errors: Vec<ConduitError>,
    initialized: Vec<i32>,
    consumed_this_round: BTreeMap<(i32, i32), i32>,
    uses_this_round: BTreeMap<i64, i32>,
    counters_this_round: BTreeMap<(i32, ConduitCounterKind), i32>,
    pending_activations: BTreeMap<(i64, i32), PendingActivation>,
    running: HashSet<i64>,
}

impl ConduitManager {
    pub fn initialization_commands(&self) -> Vec<ConduitCommand> {
        self.areas
            .keys()
            .filter(|team| !self.initialized.contains(team))
            .copied()
            .map(|team| ConduitCommand::Initialize { team })
            .collect()
    }

    pub fn opening_reset_commands(&self) -> Vec<ConduitCommand> {
        self.areas
            .iter()
            .flat_map(|(team, area)| {
                std::iter::once(ConduitCommand::ResetPowers { team: *team }).chain(
                    area.devices
                        .iter()
                        .map(|device| ConduitCommand::RestartDevice {
                            source_uid: device.uid,
                        }),
                )
            })
            .collect()
    }

    pub fn action_phase_start_commands(&self, team: i32) -> Vec<ConduitCommand> {
        self.areas
            .get(&team)
            .into_iter()
            .flat_map(|area| {
                std::iter::once(ConduitCommand::ResetPowers { team }).chain(
                    area.devices
                        .iter()
                        .map(|device| ConduitCommand::RestartDevice {
                            source_uid: device.uid,
                        }),
                )
            })
            .collect()
    }

    pub fn power(&self, team: i32, power_id: i32) -> i32 {
        self.areas
            .get(&team)
            .and_then(|area| area.powers.iter().find(|power| power.id == power_id))
            .map(|power| power.value)
            .unwrap_or_default()
    }

    pub fn begin_round(&mut self) {
        self.consumed_this_round.clear();
        self.uses_this_round.clear();
        self.counters_this_round.clear();
        self.pending_activations.clear();
        for skill in self
            .areas
            .values_mut()
            .flat_map(|area| &mut area.devices)
            .flat_map(|device| &mut device.skill_groups)
            .flatten()
        {
            skill.is_stopped = false;
        }
    }

    pub fn selected_skills(&self, source_uid: i64) -> Result<Vec<ConduitSkill>, ConduitError> {
        let device = self
            .areas
            .values()
            .flat_map(|area| &area.devices)
            .find(|device| device.uid == source_uid)
            .ok_or(ConduitError::MissingDevice(source_uid))?;
        device
            .skill_groups
            .get(device.selected_group.saturating_sub(1) as usize)
            .cloned()
            .ok_or(ConduitError::InvalidGroup {
                source_uid,
                group: device.selected_group,
            })
    }

    pub fn selected_group(&self, source_uid: i64) -> Option<i32> {
        self.areas
            .values()
            .flat_map(|area| &area.devices)
            .find(|device| device.uid == source_uid)
            .map(|device| device.selected_group)
    }

    pub fn selections(&self) -> Vec<(i64, i32)> {
        self.areas
            .values()
            .flat_map(|area| &area.devices)
            .map(|device| (device.uid, device.selected_group))
            .collect()
    }

    pub fn skill_ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.areas
            .values()
            .flat_map(|area| &area.devices)
            .flat_map(|device| &device.skill_groups)
            .flatten()
            .map(|skill| skill.skill_id)
    }

    pub fn can_begin_skill(&self, source_uid: i64, skill_id: i32, cost_reduction: i32) -> bool {
        self.skill(source_uid, skill_id)
            .is_some_and(|(team, skill)| {
                let cost = skill.cost_after_reduction(cost_reduction);
                !skill.is_stopped
                    && (skill.cost_type == 999 || self.power(team, skill.cost_type) >= cost)
            })
    }

    pub fn owns_skill(&self, source_uid: i64, skill_id: i32) -> bool {
        self.configured_skill(source_uid, skill_id).is_some()
    }

    fn configured_skill(&self, source_uid: i64, skill_id: i32) -> Option<(i32, ConduitSkill)> {
        self.areas.iter().find_map(|(team, area)| {
            let device = area
                .devices
                .iter()
                .find(|device| device.uid == source_uid)?;
            device
                .skill_groups
                .iter()
                .flatten()
                .find(|skill| skill.skill_id == skill_id)
                .copied()
                .map(|skill| (*team, skill))
        })
    }

    pub fn consumed(&self, team: i32, power_id: i32) -> i32 {
        self.consumed_this_round
            .get(&(team, power_id))
            .copied()
            .unwrap_or_default()
    }

    pub fn consumed_for_skill(&self, source_uid: i64, skill_id: i32) -> Option<i32> {
        let (team, _) = self.configured_skill(source_uid, skill_id)?;
        Some(self.counter(team, ConduitCounterKind::EnergyAccumulation))
    }

    pub fn uses(&self, source_uid: i64) -> i32 {
        self.uses_this_round
            .get(&source_uid)
            .copied()
            .unwrap_or_default()
    }

    pub fn counter(&self, team: i32, kind: ConduitCounterKind) -> i32 {
        self.counters_this_round
            .get(&(team, kind))
            .copied()
            .unwrap_or_default()
    }

    pub fn is_running(&self, source_uid: i64) -> bool {
        self.running.contains(&source_uid)
    }

    pub fn skill(&self, source_uid: i64, skill_id: i32) -> Option<(i32, ConduitSkill)> {
        self.areas.iter().find_map(|(team, area)| {
            let device = area
                .devices
                .iter()
                .find(|device| device.uid == source_uid)?;
            device
                .skill_groups
                .get(device.selected_group.saturating_sub(1) as usize)?
                .iter()
                .find(|skill| skill.skill_id == skill_id)
                .copied()
                .map(|skill| (*team, skill))
        })
    }

    fn select_group(&mut self, source_uid: i64, group: i32) -> Result<i32, ConduitError> {
        let (team, device) = self
            .areas
            .iter_mut()
            .find_map(|(team, area)| {
                area.devices
                    .iter_mut()
                    .find(|device| device.uid == source_uid)
                    .map(|device| (*team, device))
            })
            .ok_or(ConduitError::MissingDevice(source_uid))?;
        if !(1..=device.skill_groups.len() as i32).contains(&group) {
            return Err(ConduitError::InvalidGroup { source_uid, group });
        }
        device.selected_group = group;
        Ok(team)
    }
}

fn reduced_cost(cost: i32, reduction: i32) -> i32 {
    cost.saturating_sub(reduction.max(0)).max(0)
}

impl ConduitChange {
    pub fn events(&self) -> Vec<crate::engine::event::payload::BattleEvent> {
        let (source_uid, team, skill_id, power_id, activation_cost, spent) = match self {
            Self::ActivationCompleted(event) => (
                event.source_uid,
                event.team,
                event.skill_id,
                event.power_id,
                event.activation_cost,
                event.spent,
            ),
            Self::PowersCleared {
                source_uid,
                team,
                skill_id,
                spent,
                ..
            } => (*source_uid, *team, *skill_id, 0, *spent, *spent),
            _ => return Vec::new(),
        };
        (spent > 0)
            .then_some(
                crate::engine::event::payload::BattleEvent::ConduitActivated(
                    crate::engine::event::payload::ConduitActivatedEvent {
                        source_uid,
                        team,
                        skill_id,
                        power_id,
                        activation_cost,
                        spent,
                    },
                ),
            )
            .into_iter()
            .collect()
    }
}
