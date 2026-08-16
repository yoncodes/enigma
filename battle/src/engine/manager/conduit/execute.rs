use super::*;

impl ConduitManager {
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
                let accumulated = self
                    .counters_this_round
                    .entry((activation.team, ConduitCounterKind::EnergyAccumulation))
                    .or_default();
                *accumulated = accumulated.saturating_add(activation.activation_cost);
                self.pending_activations
                    .get_mut(&(source_uid, skill_id))
                    .expect("the checked activation remains pending")
                    .cost_committed = true;
                Ok(ConduitChange::SkillCostCommitted {
                    source_uid,
                    team: activation.team,
                    skill_id,
                    power_id: activation.power_id,
                    activation_cost: activation.activation_cost,
                    consumed_this_round: *accumulated,
                })
            }
            ConduitCommand::FinishSkill {
                source_uid,
                skill_id,
            } => {
                let (team, _) = self
                    .skill(source_uid, skill_id)
                    .ok_or(ConduitError::MissingSkill(skill_id))?;
                let device_uses = self.uses_this_round.entry(source_uid).or_default();
                *device_uses = device_uses.saturating_add(1);
                let uses = self
                    .counters_this_round
                    .entry((team, ConduitCounterKind::Activation))
                    .or_default();
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
            ConduitCommand::ChangeCounter(change) => {
                let area = self
                    .areas
                    .get(&change.team)
                    .ok_or(ConduitError::MissingArea(change.team))?;
                if !area
                    .devices
                    .iter()
                    .any(|device| device.uid == change.source_uid)
                {
                    return Err(ConduitError::MissingDevice(change.source_uid));
                }
                let counter = self
                    .counters_this_round
                    .entry((change.team, change.kind))
                    .or_default();
                let before = *counter;
                *counter = counter.saturating_add(change.delta).max(0);
                Ok(ConduitChange::CounterChanged {
                    origin: change.origin,
                    source_uid: change.source_uid,
                    team: change.team,
                    kind: change.kind,
                    requested_delta: change.delta,
                    applied_delta: *counter - before,
                    after: *counter,
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
}
