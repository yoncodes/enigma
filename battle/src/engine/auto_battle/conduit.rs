use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
};

use sonettobuf::{BeginRoundOper, FightDeviceOper};

use crate::engine::{
    manager::{
        BattleManagers,
        conduit::{ConduitChange, ConduitDevice, ConduitSkill},
    },
    mechanic::card::CardMechanic,
    runtime::{determinism::RoundDeterminism, executor::RuleOutcome, schedule::run_conduit_phase},
    skill::{
        behavior::classify::BehaviorKind,
        condition::parse::ParsedConditionKind,
        effect::{SkillEffectCatalog, catalog::SkillEffectTag},
        target::TargetPool,
    },
};

use super::simulation::Simulation;
use super::{
    AutoBattleError, Support, certain_buffs, is_shield_buff, support_need, target_options, workers,
};

pub(super) mod lookahead;
#[cfg(test)]
mod tests;

type GroupScore = (i32, i32, i64);

struct GroupCandidate<'a> {
    device_index: usize,
    group_index: usize,
    requested_group: i32,
    source_uid: i64,
    desired_skills: &'a [ConduitSkill],
}

struct GroupJob<'a> {
    operations: Vec<FightDeviceOper>,
    candidates: Vec<GroupCandidate<'a>>,
}

struct GroupSimulation {
    before: BattleManagers,
    catalog: SkillEffectCatalog,
    determinism: RoundDeterminism,
    outcomes: Vec<RuleOutcome>,
}

#[derive(Debug, Default)]
struct DeviceSelectionState {
    round_start_group: i32,
    switched_group: Option<i32>,
    fed_after_switch: bool,
    planned_group: Option<i32>,
}

#[derive(Debug, Default)]
pub(super) struct SelectionPlan {
    devices: HashMap<i64, DeviceSelectionState>,
}

impl SelectionPlan {
    pub(super) fn new(managers: &BattleManagers) -> Self {
        Self {
            devices: managers
                .conduit
                .devices()
                .map(|device| {
                    (
                        device.uid,
                        DeviceSelectionState {
                            round_start_group: device.selected_group,
                            ..Default::default()
                        },
                    )
                })
                .collect(),
        }
    }

    pub(super) fn set_planned(&mut self, operations: &[FightDeviceOper]) {
        for operation in operations {
            if let Some(state) = operation.uid.and_then(|uid| self.devices.get_mut(&uid)) {
                state.planned_group = operation.index;
            }
        }
    }

    pub(super) fn observe_card(
        &mut self,
        source_uid: i64,
        before_group: Option<i32>,
        after_group: Option<i32>,
        is_energy: bool,
    ) {
        if is_energy
            && let Some(state) = self
                .devices
                .get_mut(&source_uid)
                .filter(|state| state.switched_group.is_some())
        {
            state.fed_after_switch = true;
        }
        if before_group != after_group
            && let (Some(state), Some(group)) = (self.devices.get_mut(&source_uid), after_group)
        {
            state.switched_group = Some(group);
            state.fed_after_switch = false;
        }
    }

    pub(super) fn reconcile(
        &self,
        planned: &[FightDeviceOper],
        managers: &BattleManagers,
    ) -> Vec<FightDeviceOper> {
        planned
            .iter()
            .map(|operation| {
                let Some((uid, desired_group, after_cards_group)) =
                    operation.uid.zip(operation.index).and_then(|(uid, group)| {
                        Some((uid, group, managers.conduit.selected_group(uid)?))
                    })
                else {
                    return operation.clone();
                };
                FightDeviceOper {
                    uid: Some(uid),
                    index: Some(self.requested_group(uid, after_cards_group, desired_group)),
                }
            })
            .collect()
    }

    fn requested_group(&self, uid: i64, after_cards_group: i32, desired_group: i32) -> i32 {
        let Some(state) = self.devices.get(&uid) else {
            return desired_group;
        };
        if state.switched_group != Some(after_cards_group) || desired_group != after_cards_group {
            return desired_group;
        }
        let planned_group_was_fed =
            state.planned_group == Some(after_cards_group) && state.fed_after_switch;
        if planned_group_was_fed {
            desired_group
        } else {
            state.round_start_group
        }
    }
}

pub(super) struct Selection {
    pub requested: Vec<FightDeviceOper>,
    pub effective: Vec<FightDeviceOper>,
}

pub(super) fn operations(
    simulation: &Simulation<'_>,
    opers: &[BeginRoundOper],
) -> Result<Option<Selection>, AutoBattleError> {
    let devices = simulation
        .managers
        .conduit
        .devices()
        .cloned()
        .collect::<Vec<_>>();
    let mut operations = devices
        .iter()
        .map(|device| FightDeviceOper {
            uid: Some(device.uid),
            index: Some(device.selected_group),
        })
        .collect::<Vec<_>>();
    let initial_pool = simulation.pool.runtime_view(&simulation.managers);
    let available = devices
        .iter()
        .flat_map(|device| {
            let source = initial_pool.entity(device.uid);
            device
                .skill_groups
                .iter()
                .enumerate()
                .filter(move |(_, skills)| {
                    !skills
                        .iter()
                        .any(|skill| simulation.managers.catalog().skill_is_big(skill.skill_id))
                        || source.is_none_or(|source| {
                            CardMechanic.ultimate_resource_ready(&simulation.managers, source)
                        })
                })
                .map(move |(index, _)| (device.uid, index + 1))
        })
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    let mut simulations = Vec::new();
    loop {
        record_groups(&mut seen, &operations)?;
        let jobs = group_jobs(&devices, &operations, &available);
        if !simulations
            .iter()
            .any(|(candidate, _)| candidate == &operations)
        {
            simulations.push((
                operations.clone(),
                simulate_operations(simulation, opers, &operations),
            ));
        }
        let current = simulations
            .iter()
            .find(|(candidate, _)| candidate == &operations)
            .and_then(|(_, simulated)| simulated.as_ref());
        let mut current_scores = vec![None; devices.len()];
        if let Some(job) = jobs.iter().find(|job| job.operations == operations) {
            for candidate in &job.candidates {
                current_scores[candidate.device_index] =
                    current.and_then(|simulated| score_group(simulation, simulated, candidate));
            }
        }
        let missing = jobs
            .iter()
            .filter(|job| {
                !simulations
                    .iter()
                    .any(|(operations, _)| operations == &job.operations)
            })
            .filter(|job| {
                job.candidates.iter().any(|candidate| {
                    priority_can_beat(
                        current_scores[candidate.device_index],
                        candidate
                            .desired_skills
                            .iter()
                            .filter(|skill| {
                                simulation.managers.catalog().skill_is_big(skill.skill_id)
                            })
                            .count() as i32,
                    )
                })
            })
            .map(|job| job.operations.clone())
            .collect::<Vec<_>>();
        let added = if missing.is_empty() {
            Vec::new()
        } else {
            workers::run(missing.len(), |worker_count| {
                let chunk_size = missing.len().div_ceil(worker_count);
                std::thread::scope(|scope| {
                    missing
                        .chunks(chunk_size)
                        .map(|operations| {
                            scope.spawn(move || {
                                operations
                                    .iter()
                                    .map(|operations| {
                                        (
                                            operations.clone(),
                                            simulate_operations(simulation, opers, operations),
                                        )
                                    })
                                    .collect::<Vec<_>>()
                            })
                        })
                        .collect::<Vec<_>>()
                        .into_iter()
                        .flat_map(|worker| worker.join().expect("conduit scoring worker panicked"))
                        .collect::<Vec<_>>()
                })
            })
        };
        simulations.extend(added);
        let candidates = jobs
            .iter()
            .flat_map(|job| {
                let simulated = simulations
                    .iter()
                    .find(|(operations, _)| operations == &job.operations)
                    .and_then(|(_, simulated)| simulated.as_ref());
                job.candidates.iter().map(move |candidate| {
                    (
                        candidate.device_index,
                        candidate.group_index,
                        candidate.requested_group,
                        simulated
                            .and_then(|simulated| score_group(simulation, simulated, candidate)),
                    )
                })
            })
            .collect::<Vec<_>>();
        let mut next = operations.clone();
        for (device_index, device) in devices.iter().enumerate() {
            let mut best = None;
            for (_, index, requested_group, candidate) in candidates
                .iter()
                .filter(|(candidate_index, ..)| *candidate_index == device_index)
            {
                let Some(candidate) = candidate else {
                    continue;
                };
                let key = (*candidate, Reverse(*index));
                if best.as_ref().is_none_or(|(best_key, _)| key > *best_key) {
                    best = Some((key, *requested_group));
                }
            }
            next[device_index].index = Some(best.map_or(device.selected_group, |(_, group)| group));
        }
        if next == operations {
            break;
        }
        operations = next;
    }

    let Some((after_cards, _)) = simulation.player(opers, &operations) else {
        return Ok(None);
    };
    Ok(Some(Selection {
        requested: operations,
        effective: after_cards
            .conduit
            .selections()
            .into_iter()
            .map(|(uid, group)| FightDeviceOper {
                uid: Some(uid),
                index: Some(group),
            })
            .collect(),
    }))
}

fn group_jobs<'a>(
    devices: &'a [ConduitDevice],
    operations: &[FightDeviceOper],
    available: &HashSet<(i64, usize)>,
) -> Vec<GroupJob<'a>> {
    let mut jobs: Vec<GroupJob<'a>> = Vec::new();
    for (device_index, device) in devices.iter().enumerate() {
        for (group_index, desired_skills) in device.skill_groups.iter().enumerate() {
            if !available.contains(&(device.uid, group_index + 1)) {
                continue;
            }
            let requested_group = group_index as i32 + 1;
            let mut candidate_operations = operations.to_vec();
            candidate_operations[device_index].index = Some(requested_group);
            let candidate = GroupCandidate {
                device_index,
                group_index,
                requested_group,
                source_uid: device.uid,
                desired_skills,
            };
            if let Some(job) = jobs
                .iter_mut()
                .find(|job| job.operations == candidate_operations)
            {
                job.candidates.push(candidate);
            } else {
                jobs.push(GroupJob {
                    operations: candidate_operations,
                    candidates: vec![candidate],
                });
            }
        }
    }
    jobs
}

fn record_groups(
    seen: &mut HashSet<Vec<Option<i32>>>,
    operations: &[FightDeviceOper],
) -> Result<(), AutoBattleError> {
    seen.insert(operations.iter().map(|operation| operation.index).collect())
        .then_some(())
        .ok_or(AutoBattleError::OscillatingConduitGroups)
}

fn priority_can_beat(current: Option<GroupScore>, candidate_max: i32) -> bool {
    current.is_none_or(|score| score.0 <= candidate_max)
}

pub(super) fn selected_power_types(
    managers: &BattleManagers,
    operations: &[FightDeviceOper],
) -> HashSet<i32> {
    operations
        .iter()
        .filter_map(|operation| Some((operation.uid?, operation.index?)))
        .filter_map(|(uid, group)| {
            managers
                .conduit
                .devices()
                .find(|device| device.uid == uid)?
                .skill_groups
                .get(group.saturating_sub(1) as usize)
        })
        .flatten()
        .filter(|skill| skill.cost_type > 0 && skill.cost_type != 999 && skill.cost_value > 0)
        .map(|skill| skill.cost_type)
        .collect()
}

fn simulate_operations(
    simulation: &Simulation<'_>,
    opers: &[BeginRoundOper],
    operations: &[FightDeviceOper],
) -> Option<GroupSimulation> {
    let (before, mut sim_determinism) = simulation.player(opers, operations)?;
    let mut sim = before.clone();
    let mut sim_catalog = simulation.catalog.clone();
    let skills = before
        .conduit
        .devices()
        .map(|device| before.conduit.selected_skills(device.uid))
        .collect::<Result<Vec<_>, _>>()
        .inspect_err(|error| tracing::warn!(?error, "auto-battle rejected conduit selection"))
        .ok()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    sim.catalog().extend_skill_roots(
        &mut sim_catalog,
        skills.iter().map(|skill| skill.skill_id),
        std::iter::empty(),
    );
    if let Some(skill) = skills.iter().find(|skill| {
        sim_catalog.get(skill.skill_id).is_none() || !sim_catalog.issues(skill.skill_id).is_empty()
    }) {
        tracing::warn!(
            skill_id = skill.skill_id,
            issues = ?sim_catalog.issues(skill.skill_id),
            "auto-battle rejected unsupported conduit group"
        );
        return None;
    }
    let context = crate::engine::runtime::selected_enemy_target(
        &crate::engine::round::command::commands_from_opers(opers),
        &simulation.pool,
    )
    .map(
        |runtime_target_uid| crate::engine::skill::target::TargetContext {
            runtime_target_uid,
            ..simulation.context
        },
    )
    .unwrap_or(simulation.context);
    let before_determinism = sim_determinism.clone();
    let result = run_conduit_phase(
        sim.catalog(),
        &simulation.fight,
        &mut sim,
        &simulation.pool,
        &mut sim_catalog,
        &mut sim_determinism,
        context,
        operations,
    )
    .inspect_err(|error| tracing::warn!(?error, "auto-battle rejected conduit group"))
    .ok()?;
    Some(GroupSimulation {
        before,
        catalog: sim_catalog,
        determinism: before_determinism,
        outcomes: result.outcomes,
    })
}

fn score_group(
    simulation: &Simulation<'_>,
    simulated: &GroupSimulation,
    candidate: &GroupCandidate<'_>,
) -> Option<GroupScore> {
    let pre_pool = simulation.pool.runtime_view(&simulated.before);
    let source = pre_pool.entity(candidate.source_uid)?;
    if simulated
        .before
        .conduit
        .selected_skills(candidate.source_uid)
        .ok()?
        .iter()
        .any(|skill| simulated.catalog.is_big_skill(skill.skill_id))
        && !CardMechanic.ultimate_resource_ready(&simulated.before, source)
    {
        return None;
    }
    let score = executed_skills(
        &simulated.outcomes,
        candidate.source_uid,
        candidate.desired_skills,
    )
    .into_iter()
    .map(|(skill, spent)| {
        skill_score(
            candidate.source_uid,
            skill,
            spent,
            &simulated.before,
            &pre_pool,
            &simulated.catalog,
            &simulated.determinism,
        )
    })
    .fold((0, 0, 0), |total, score| {
        (total.0 + score.0, total.1 + score.1, total.2 + score.2)
    });
    (score != (0, 0, 0)).then_some(score)
}

fn executed_skills(
    outcomes: &[RuleOutcome],
    source_uid: i64,
    skills: &[ConduitSkill],
) -> Vec<(ConduitSkill, i32)> {
    let mut seen = HashSet::new();
    outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            RuleOutcome::Conduit(ConduitChange::SkillBegan {
                source_uid: owner_uid,
                skill_id,
                spent,
                ..
            }) if *owner_uid == source_uid && seen.insert(*skill_id) => skills
                .iter()
                .find(|skill| skill.skill_id == *skill_id)
                .map(|skill| (*skill, *spent)),
            _ => None,
        })
        .collect()
}

fn skill_score(
    source_uid: i64,
    skill: ConduitSkill,
    spent: i32,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
) -> GroupScore {
    let Some(source) = pool.entity(source_uid) else {
        return (0, 0, 0);
    };
    let targets = target_options(
        source_uid,
        skill.skill_id,
        None,
        managers,
        pool,
        catalog,
        determinism,
    );
    let buffs = if catalog.is_attack(skill.skill_id) {
        Vec::new()
    } else {
        certain_buffs(skill.skill_id, catalog)
    };
    let support = conduit_support_need(
        catalog.effect_tag(skill.skill_id) == SkillEffectTag::Heal as i32,
        &buffs,
        &targets,
        managers,
        pool,
    );
    let support_score = match support {
        Support::Urgent => 2,
        Support::None
            if !buffs.is_empty() && targets.iter().any(|target| *target != source_uid) =>
        {
            1
        }
        _ => 0,
    };
    let attack_score = if catalog.is_attack(skill.skill_id) {
        let attack_career = catalog
            .get(skill.skill_id)
            .into_iter()
            .flat_map(|effect| &effect.slots)
            .find(|slot| {
                slot.behavior.spec.kind == BehaviorKind::ChangeAttackCareer
                    && slot
                        .conditions
                        .iter()
                        .all(|condition| matches!(condition.kind, ParsedConditionKind::None(_)))
            })
            .and_then(|slot| slot.behavior.args.first())
            .copied()
            .unwrap_or(source.career);
        let affinity = targets
            .iter()
            .filter_map(|uid| pool.entity(*uid))
            .map(|target| {
                managers
                    .catalog()
                    .career_multiplier(attack_career, target.career)
            })
            .max()
            .unwrap_or_default();
        i64::from(catalog.damage_rate(skill.skill_id)) * i64::from(affinity)
            / 1_000
            / i64::from(spent + 1)
    } else {
        0
    };
    (
        i32::from(catalog.is_big_skill(skill.skill_id)),
        support_score,
        attack_score,
    )
}

fn conduit_support_need(
    heal: bool,
    buffs: &[i32],
    targets: &[i64],
    managers: &BattleManagers,
    pool: &TargetPool,
) -> Support {
    if heal {
        return support_need(true, false, &[], targets, managers, pool);
    }
    if !buffs.is_empty()
        && targets.iter().all(|uid| {
            buffs
                .iter()
                .all(|buff_id| conduit_buff_held(*uid, *buff_id, managers))
        })
    {
        Support::Unneeded
    } else {
        Support::None
    }
}

fn conduit_buff_held(uid: i64, buff_id: i32, managers: &BattleManagers) -> bool {
    !is_shield_buff(buff_id, managers)
        && managers
            .buff
            .buff_family_carrier_uid(uid, buff_id)
            .and_then(|buff_uid| managers.buff.snapshot(uid, buff_uid))
            .is_some_and(|buff| buff.duration.is_none_or(|duration| duration > 0))
}
