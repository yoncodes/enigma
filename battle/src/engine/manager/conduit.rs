use std::collections::{BTreeMap, HashSet};

use sonettobuf::{Fight, FightEntityInfo};

use crate::engine::skill::rule::CommandOrigin;

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
    pending_activations: BTreeMap<(i64, i32), PendingActivation>,
    running: HashSet<i64>,
}

impl ConduitManager {
    pub fn seed(fight: &Fight) -> Self {
        let mut manager = Self::default();
        for (team, fight_team) in [(1, fight.attacker.as_ref()), (2, fight.defender.as_ref())] {
            let Some(fight_team) = fight_team else {
                continue;
            };
            for entity in &fight_team.entitys {
                manager.seed_entity(team, entity);
            }
        }
        manager
    }

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
        std::iter::once(ConduitCommand::ResetPowers { team })
            .chain(
                self.areas
                    .get(&team)
                    .into_iter()
                    .flat_map(|area| &area.devices)
                    .map(|device| ConduitCommand::RestartDevice {
                        source_uid: device.uid,
                    }),
            )
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
        let (team, skill) = self.configured_skill(source_uid, skill_id)?;
        Some(self.consumed(team, skill.cost_type))
    }

    pub fn uses(&self, source_uid: i64) -> i32 {
        self.uses_this_round
            .get(&source_uid)
            .copied()
            .unwrap_or_default()
    }

    pub fn is_running(&self, source_uid: i64) -> bool {
        self.running.contains(&source_uid)
    }

    pub fn execute(&mut self, command: ConduitCommand) -> Result<ConduitChange, ConduitError> {
        if let Some(error) = self.initialization_errors.first() {
            return Err(*error);
        }
        match command {
            ConduitCommand::Initialize { team } => {
                if self.initialized.contains(&team) {
                    return Err(ConduitError::AlreadyInitialized(team));
                }
                let area = self
                    .areas
                    .get(&team)
                    .cloned()
                    .ok_or(ConduitError::MissingArea(team))?;
                self.initialized.push(team);
                Ok(ConduitChange::Initialized(area))
            }
            ConduitCommand::SelectGroup { source_uid, group } => {
                let team = self.select_group(source_uid, group)?;
                Ok(ConduitChange::GroupSelected {
                    source_uid,
                    team,
                    group,
                })
            }
            ConduitCommand::SetSkillGroup {
                origin,
                source_uid,
                group,
            } => {
                let team = self.select_group(source_uid, group)?;
                Ok(ConduitChange::SkillGroupChanged {
                    origin,
                    source_uid,
                    team,
                    group,
                })
            }
            ConduitCommand::BeginSkill {
                source_uid,
                skill_id,
                cost_reduction,
            } => {
                if self
                    .pending_activations
                    .contains_key(&(source_uid, skill_id))
                {
                    return Err(ConduitError::ActivationInProgress(skill_id));
                }
                let (team, skill) = self
                    .skill(source_uid, skill_id)
                    .ok_or(ConduitError::MissingSkill(skill_id))?;
                if skill.is_stopped {
                    return Err(ConduitError::StoppedSkill(skill_id));
                }
                let spent = skill.cost_after_reduction(cost_reduction);
                let available = self.power(team, skill.cost_type);
                if skill.cost_type != 999 && available < spent {
                    return Err(ConduitError::InsufficientPower {
                        power_id: skill.cost_type,
                        available,
                        required: spent,
                    });
                }
                if skill.cost_type != 999 {
                    let area = self
                        .areas
                        .get_mut(&team)
                        .ok_or(ConduitError::MissingArea(team))?;
                    let power = area
                        .powers
                        .iter_mut()
                        .find(|power| power.id == skill.cost_type);
                    if let Some(power) = power {
                        power.value -= spent;
                    } else if spent > 0 {
                        return Err(ConduitError::InsufficientPower {
                            power_id: skill.cost_type,
                            available: 0,
                            required: spent,
                        });
                    }
                }
                self.pending_activations.insert(
                    (source_uid, skill_id),
                    PendingActivation {
                        event: crate::engine::event::payload::ConduitActivatedEvent {
                            source_uid,
                            team,
                            skill_id,
                            power_id: skill.cost_type,
                            activation_cost: skill.cost_value,
                            spent,
                        },
                        cost_committed: false,
                    },
                );
                Ok(ConduitChange::SkillBegan {
                    source_uid,
                    team,
                    skill_id,
                    power_id: skill.cost_type,
                    activation_cost: skill.cost_value,
                    spent,
                })
            }
            ConduitCommand::CommitSkillCost {
                source_uid,
                skill_id,
            } => {
                let pending = self
                    .pending_activations
                    .get(&(source_uid, skill_id))
                    .copied()
                    .ok_or(ConduitError::MissingActivation(skill_id))?;
                if pending.cost_committed {
                    return Err(ConduitError::ActivationAlreadyCommitted(skill_id));
                }
                let activation = pending.event;
                let consumed = self
                    .consumed_this_round
                    .entry((activation.team, activation.power_id))
                    .or_default();
                *consumed = consumed.saturating_add(activation.activation_cost);
                self.pending_activations
                    .get_mut(&(source_uid, skill_id))
                    .expect("the checked activation remains pending")
                    .cost_committed = true;
                Ok(ConduitChange::SkillCostCommitted {
                    source_uid,
                    team: activation.team,
                    skill_id,
                    activation_cost: activation.activation_cost,
                    consumed_this_round: *consumed,
                })
            }
            ConduitCommand::FinishSkill {
                source_uid,
                skill_id,
            } => {
                let (team, _) = self
                    .skill(source_uid, skill_id)
                    .ok_or(ConduitError::MissingSkill(skill_id))?;
                let uses = self.uses_this_round.entry(source_uid).or_default();
                *uses = uses.saturating_add(1);
                Ok(ConduitChange::SkillFinished {
                    source_uid,
                    team,
                    skill_id,
                    uses_this_round: *uses,
                })
            }
            ConduitCommand::CompleteActivation {
                source_uid,
                skill_id,
            } => {
                let pending = self
                    .pending_activations
                    .get(&(source_uid, skill_id))
                    .copied()
                    .ok_or(ConduitError::MissingActivation(skill_id))?;
                if !pending.cost_committed {
                    return Err(ConduitError::ActivationNotCommitted(skill_id));
                }
                self.pending_activations.remove(&(source_uid, skill_id));
                Ok(ConduitChange::ActivationCompleted(pending.event))
            }
            ConduitCommand::SetRunning {
                source_uid,
                running,
            } => {
                self.areas
                    .values()
                    .flat_map(|area| &area.devices)
                    .find(|device| device.uid == source_uid)
                    .ok_or(ConduitError::MissingDevice(source_uid))?;
                if running {
                    self.running.insert(source_uid);
                } else {
                    self.running.remove(&source_uid);
                }
                Ok(ConduitChange::RunningChanged {
                    source_uid,
                    running,
                })
            }
            ConduitCommand::ChangePower(change) => {
                let area = self
                    .areas
                    .get_mut(&change.team)
                    .ok_or(ConduitError::MissingArea(change.team))?;
                let power = match area
                    .powers
                    .iter_mut()
                    .find(|power| power.id == change.power_id)
                {
                    Some(power) => power,
                    None => {
                        area.powers.push(ConduitPower {
                            id: change.power_id,
                            value: 0,
                        });
                        area.powers.last_mut().expect("a Conduit power was added")
                    }
                };
                let before = power.value;
                power.value = power.value.saturating_add(change.delta).max(0);
                Ok(ConduitChange::PowerChanged {
                    origin: change.origin,
                    source_uid: change.source_uid,
                    team: change.team,
                    power_id: change.power_id,
                    requested_delta: change.delta,
                    applied_delta: power.value - before,
                    after: power.value,
                    kind: change.kind,
                })
            }
            ConduitCommand::ClearPowers {
                origin,
                source_uid,
                team,
                skill_id,
                power_ids,
            } => {
                let area = self
                    .areas
                    .get_mut(&team)
                    .ok_or(ConduitError::MissingArea(team))?;
                let mut spent = 0i32;
                for power_id in power_ids {
                    let value = area
                        .powers
                        .iter_mut()
                        .find(|power| power.id == power_id)
                        .map(|power| std::mem::take(&mut power.value))
                        .unwrap_or_default()
                        .max(0);
                    spent = spent.saturating_add(value);
                    self.consumed_this_round
                        .entry((team, power_id))
                        .and_modify(|consumed| *consumed = consumed.saturating_add(value))
                        .or_insert(value);
                }
                Ok(ConduitChange::PowersCleared {
                    origin,
                    source_uid,
                    team,
                    skill_id,
                    power_ids,
                    spent,
                })
            }
            ConduitCommand::ResetPowers { team } => {
                if let Some(area) = self.areas.get_mut(&team) {
                    for power in &mut area.powers {
                        power.value = 0;
                    }
                }
                Ok(ConduitChange::PowersReset { team })
            }
            ConduitCommand::StopSkill {
                origin,
                source_uid,
                team,
                skill_id,
            } => {
                let area = self
                    .areas
                    .get_mut(&team)
                    .ok_or(ConduitError::MissingArea(team))?;
                let device = area
                    .devices
                    .iter_mut()
                    .find(|device| device.uid == source_uid)
                    .ok_or(ConduitError::MissingDevice(source_uid))?;
                let skill = device
                    .skill_groups
                    .iter_mut()
                    .flatten()
                    .find(|skill| skill.skill_id == skill_id)
                    .ok_or(ConduitError::MissingSkill(skill_id))?;
                skill.is_stopped = true;
                Ok(ConduitChange::SkillStopped {
                    origin,
                    source_uid,
                    team,
                    skill_id,
                })
            }
            ConduitCommand::RestartDevice { source_uid } => {
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
                for skill in device.skill_groups.iter_mut().flatten() {
                    skill.is_stopped = false;
                }
                Ok(ConduitChange::DeviceRestarted { source_uid, team })
            }
        }
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

    fn seed_entity(&mut self, team: i32, entity: &FightEntityInfo) {
        let (Some(uid), Some(model_id)) = (entity.uid, entity.model_id) else {
            return;
        };
        let configs = config::configs::get();
        let Some(character) = configs.character.get(model_id) else {
            return;
        };
        if character.device_id == 0 {
            return;
        }
        let Some(definition) = configs.fight_device.get(character.device_id) else {
            self.initialization_errors
                .push(ConduitError::MissingDefinition(character.device_id));
            return;
        };
        let groups = [
            parse_skill_group(
                character.device_id,
                ConduitSkillGroup::Primary,
                &definition.skill1,
            ),
            parse_skill_group(
                character.device_id,
                ConduitSkillGroup::Secondary,
                &definition.skill2,
            ),
            parse_unique_skill(character.device_id, &definition.unique_skill),
        ];
        let mut skill_groups = Vec::with_capacity(groups.len());
        for group in groups {
            match group {
                Ok(group) => skill_groups.push(group),
                Err(error) => {
                    self.initialization_errors.push(error);
                    return;
                }
            }
        }
        self.areas
            .entry(team)
            .or_insert_with(|| ConduitArea {
                team,
                devices: Vec::new(),
                powers: Vec::new(),
            })
            .devices
            .push(ConduitDevice {
                uid,
                selected_group: 1,
                skill_groups,
            });
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

fn parse_skill_group(
    device_id: i32,
    group: ConduitSkillGroup,
    value: &str,
) -> Result<Vec<ConduitSkill>, ConduitError> {
    value
        .split('|')
        .map(|entry| {
            let parts = entry.split('#').collect::<Vec<_>>();
            if parts.len() != 3 {
                return Err(invalid_skill(device_id, group));
            }
            Ok(ConduitSkill {
                skill_id: parse_part(device_id, group, parts[0])?,
                cost_type: parse_part(device_id, group, parts[1])?,
                cost_value: parse_part(device_id, group, parts[2])?,
                is_stopped: false,
            })
        })
        .collect()
}

fn parse_unique_skill(device_id: i32, value: &str) -> Result<Vec<ConduitSkill>, ConduitError> {
    Ok(vec![ConduitSkill {
        skill_id: parse_part(device_id, ConduitSkillGroup::Unique, value)?,
        cost_type: 999,
        cost_value: 0,
        is_stopped: false,
    }])
}

fn parse_part(device_id: i32, group: ConduitSkillGroup, part: &str) -> Result<i32, ConduitError> {
    part.parse().map_err(|_| invalid_skill(device_id, group))
}

fn invalid_skill(device_id: i32, group: ConduitSkillGroup) -> ConduitError {
    ConduitError::InvalidSkill { device_id, group }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::skill::rule::{DefinitionKey, RuleDomain};
    use sonettobuf::{FightEntityInfo, FightTeam};

    const ORIGIN: CommandOrigin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60291, "AddDevicePower"),
    };

    #[test]
    fn parses_configured_skill_group_without_losing_cost_identity() {
        assert_eq!(
            parse_skill_group(1, ConduitSkillGroup::Primary, "31490111#1#0|31490121#1#3",).unwrap(),
            vec![
                ConduitSkill {
                    skill_id: 31490111,
                    cost_type: 1,
                    cost_value: 0,
                    is_stopped: false,
                },
                ConduitSkill {
                    skill_id: 31490121,
                    cost_type: 1,
                    cost_value: 3,
                    is_stopped: false,
                },
            ]
        );
    }

    #[test]
    fn selects_the_requested_group_on_the_owning_device() {
        config::init(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../data/excel2json")
                .to_str()
                .unwrap(),
        )
        .ok();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3149),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut manager = ConduitManager::seed(&fight);

        assert_eq!(
            manager
                .execute(ConduitCommand::SelectGroup {
                    source_uid: 10,
                    group: 2,
                })
                .unwrap(),
            ConduitChange::GroupSelected {
                source_uid: 10,
                team: 1,
                group: 2,
            }
        );
    }

    #[test]
    fn device_skill_spends_its_configured_power_and_updates_round_counters() {
        config::init(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../data/excel2json")
                .to_str()
                .unwrap(),
        )
        .ok();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3149),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut manager = ConduitManager::seed(&fight);
        manager
            .execute(ConduitCommand::ChangePower(ConduitPowerChange {
                origin: ORIGIN,
                source_uid: 10,
                team: 1,
                power_id: 1,
                delta: 4,
                kind: ConduitPowerChangeKind::Interval,
            }))
            .unwrap();

        manager
            .execute(ConduitCommand::BeginSkill {
                source_uid: 10,
                skill_id: 31490121,
                cost_reduction: 0,
            })
            .unwrap();
        manager
            .execute(ConduitCommand::CommitSkillCost {
                source_uid: 10,
                skill_id: 31490121,
            })
            .unwrap();
        manager
            .execute(ConduitCommand::CompleteActivation {
                source_uid: 10,
                skill_id: 31490121,
            })
            .unwrap();
        manager
            .execute(ConduitCommand::FinishSkill {
                source_uid: 10,
                skill_id: 31490121,
            })
            .unwrap();

        assert_eq!(manager.power(1, 1), 1);
        assert_eq!(manager.consumed(1, 1), 3);
        assert_eq!(manager.uses(10), 1);
    }

    #[test]
    fn reduced_spend_keeps_the_configured_activation_cost() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3149),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut manager = ConduitManager::seed(&fight);
        manager
            .execute(ConduitCommand::ChangePower(ConduitPowerChange {
                origin: ORIGIN,
                source_uid: 10,
                team: 1,
                power_id: 1,
                delta: 3,
                kind: ConduitPowerChangeKind::Standard,
            }))
            .unwrap();

        manager
            .execute(ConduitCommand::BeginSkill {
                source_uid: 10,
                skill_id: 31490121,
                cost_reduction: 1,
            })
            .unwrap();
        assert_eq!(
            manager.execute(ConduitCommand::BeginSkill {
                source_uid: 10,
                skill_id: 31490121,
                cost_reduction: 1,
            }),
            Err(ConduitError::ActivationInProgress(31490121))
        );
        assert_eq!(
            manager.execute(ConduitCommand::CompleteActivation {
                source_uid: 10,
                skill_id: 31490121,
            }),
            Err(ConduitError::ActivationNotCommitted(31490121))
        );
        manager
            .execute(ConduitCommand::CommitSkillCost {
                source_uid: 10,
                skill_id: 31490121,
            })
            .unwrap();
        assert_eq!(
            manager.execute(ConduitCommand::CommitSkillCost {
                source_uid: 10,
                skill_id: 31490121,
            }),
            Err(ConduitError::ActivationAlreadyCommitted(31490121))
        );
        let change = manager
            .execute(ConduitCommand::CompleteActivation {
                source_uid: 10,
                skill_id: 31490121,
            })
            .unwrap();

        assert_eq!(manager.power(1, 1), 1);
        assert_eq!(manager.consumed(1, 1), 3);
        assert!(matches!(
            change.events().as_slice(),
            [crate::engine::event::payload::BattleEvent::ConduitActivated(event)]
                if event.activation_cost == 3 && event.spent == 2
        ));
    }

    #[test]
    fn unique_skill_clears_both_energy_pools_as_one_activation() {
        config::init(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../data/excel2json")
                .to_str()
                .unwrap(),
        )
        .ok();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3149),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut manager = ConduitManager::seed(&fight);
        for (power_id, delta) in [(1, 5), (2, 7)] {
            manager
                .execute(ConduitCommand::ChangePower(ConduitPowerChange {
                    origin: ORIGIN,
                    source_uid: 10,
                    team: 1,
                    power_id,
                    delta,
                    kind: ConduitPowerChangeKind::Standard,
                }))
                .unwrap();
        }
        manager
            .execute(ConduitCommand::SetSkillGroup {
                origin: ORIGIN,
                source_uid: 10,
                group: 3,
            })
            .unwrap();

        assert!(manager.can_begin_skill(10, 31490151, 0));
        let began = manager
            .execute(ConduitCommand::BeginSkill {
                source_uid: 10,
                skill_id: 31490151,
                cost_reduction: 0,
            })
            .unwrap();
        assert!(matches!(
            began,
            ConduitChange::SkillBegan {
                power_id: 999,
                spent: 0,
                ..
            }
        ));

        let cleared = manager
            .execute(ConduitCommand::ClearPowers {
                origin: ORIGIN,
                source_uid: 10,
                team: 1,
                skill_id: 31490151,
                power_ids: [1, 2],
            })
            .unwrap();
        assert_eq!(manager.power(1, 1), 0);
        assert_eq!(manager.power(1, 2), 0);
        assert!(matches!(
            cleared,
            ConduitChange::PowersCleared { spent: 12, .. }
        ));
        assert!(matches!(
            cleared.events().as_slice(),
            [crate::engine::event::payload::BattleEvent::ConduitActivated(event)]
                if event.spent == 12 && event.skill_id == 31490151
        ));
    }

    #[test]
    fn opening_reset_clears_power_and_restarts_each_device() {
        config::init(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../data/excel2json")
                .to_str()
                .unwrap(),
        )
        .ok();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3149),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut manager = ConduitManager::seed(&fight);
        manager
            .execute(ConduitCommand::ChangePower(ConduitPowerChange {
                origin: ORIGIN,
                source_uid: 10,
                team: 1,
                power_id: 1,
                delta: 4,
                kind: ConduitPowerChangeKind::Standard,
            }))
            .unwrap();
        manager
            .execute(ConduitCommand::StopSkill {
                origin: ORIGIN,
                source_uid: 10,
                team: 1,
                skill_id: 31490121,
            })
            .unwrap();
        assert!(!manager.can_begin_skill(10, 31490121, 0));

        let changes = manager
            .opening_reset_commands()
            .into_iter()
            .map(|command| manager.execute(command).unwrap())
            .collect::<Vec<_>>();

        assert_eq!(manager.power(1, 1), 0);
        manager
            .execute(ConduitCommand::ChangePower(ConduitPowerChange {
                origin: ORIGIN,
                source_uid: 10,
                team: 1,
                power_id: 1,
                delta: 4,
                kind: ConduitPowerChangeKind::Standard,
            }))
            .unwrap();
        assert!(manager.can_begin_skill(10, 31490121, 0));
        assert!(matches!(
            changes.as_slice(),
            [
                ConduitChange::PowersReset { team: 1 },
                ConduitChange::DeviceRestarted {
                    source_uid: 10,
                    team: 1
                }
            ]
        ));
    }
}
