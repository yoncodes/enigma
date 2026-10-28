use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
};

use sonettobuf::{Fight, FightDeviceOper};

use crate::engine::{
    manager::{
        BattleManagers,
        conduit::{ConduitChange, ConduitCommand, ConduitDevice, ConduitSkill},
    },
    mechanic::card::CardMechanic,
    runtime::{determinism::RoundDeterminism, executor::RuleOutcome, schedule::run_conduit_phase},
    skill::{
        behavior::classify::BehaviorKind,
        condition::parse::ParsedConditionKind,
        effect::{SkillEffectCatalog, catalog::SkillEffectTag},
        target::{TargetContext, TargetPool},
    },
};

use super::{Support, certain_buffs, is_shield_buff, support_need, target_options};

pub(super) mod lookahead;
#[cfg(test)]
mod tests;

type GroupScore = (i32, i32, i64);

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

    fn rendered_group(&self, uid: i64, after_cards_group: i32, desired_group: i32) -> i32 {
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

pub(super) fn operations(
    fight: &Fight,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
) -> Vec<FightDeviceOper> {
    planned_operations(fight, managers, pool, catalog, determinism, None)
}

fn planned_operations(
    fight: &Fight,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
    selection_plan: Option<&SelectionPlan>,
) -> Vec<FightDeviceOper> {
    let devices = managers.conduit.devices().cloned().collect::<Vec<_>>();
    let mut operations = Vec::with_capacity(devices.len());

    for device in devices {
        let mut best = None;
        for (index, skills) in device.skill_groups.iter().enumerate() {
            let group = index as i32 + 1;
            let Some(candidate) = simulate_group(
                fight,
                managers,
                pool,
                catalog,
                determinism,
                &device,
                group,
                skills,
            ) else {
                continue;
            };
            let key = (candidate, Reverse(index));
            if best.is_none_or(|best_key| key > best_key) {
                best = Some(key);
            }
        }

        let desired_group = best
            .map(|(.., Reverse(index))| index as i32 + 1)
            .unwrap_or(device.selected_group);
        let index = selection_plan.map_or(desired_group, |plan| {
            plan.rendered_group(device.uid, device.selected_group, desired_group)
        });
        operations.push(FightDeviceOper {
            uid: Some(device.uid),
            index: Some(index),
        });
    }

    operations
}

#[allow(clippy::too_many_arguments)]
pub(super) fn selection_operations(
    fight: &Fight,
    after_cards: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
    selection_plan: &SelectionPlan,
) -> Vec<FightDeviceOper> {
    planned_operations(
        fight,
        after_cards,
        pool,
        catalog,
        determinism,
        Some(selection_plan),
    )
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

#[allow(clippy::too_many_arguments)]
fn simulate_group(
    fight: &Fight,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
    device: &ConduitDevice,
    group: i32,
    skills: &[ConduitSkill],
) -> Option<GroupScore> {
    if skills.is_empty() {
        return None;
    }
    let mut sim = managers.clone();
    let mut sim_catalog = catalog.clone();
    let mut sim_determinism = determinism.clone();
    if let Err(error) = sim.conduit.execute(ConduitCommand::SelectGroup {
        source_uid: device.uid,
        group,
    }) {
        tracing::warn!(
            source_uid = device.uid,
            group,
            ?error,
            "auto-battle rejected conduit group"
        );
        return None;
    }
    sim.catalog().extend_skill_roots(
        &mut sim_catalog,
        skills.iter().map(|skill| skill.skill_id),
        std::iter::empty(),
    );
    let current_pool = pool.runtime_view(managers);
    let source = current_pool.entity(device.uid)?;
    if skills
        .iter()
        .any(|skill| sim_catalog.is_big_skill(skill.skill_id))
        && !CardMechanic.ultimate_resource_ready(managers, source)
    {
        return None;
    }
    if let Some(skill) = skills.iter().find(|skill| {
        sim_catalog.get(skill.skill_id).is_none() || !sim_catalog.issues(skill.skill_id).is_empty()
    }) {
        tracing::warn!(
            source_uid = device.uid,
            group,
            skill_id = skill.skill_id,
            issues = ?sim_catalog.issues(skill.skill_id),
            "auto-battle rejected unsupported conduit group"
        );
        return None;
    }
    let operation = FightDeviceOper {
        uid: Some(device.uid),
        index: Some(group),
    };
    let result = run_conduit_phase(
        sim.catalog(),
        fight,
        &mut sim,
        pool,
        &mut sim_catalog,
        &mut sim_determinism,
        TargetContext::default(),
        std::slice::from_ref(&operation),
    )
    .inspect_err(|error| {
        tracing::warn!(
            source_uid = device.uid,
            group,
            ?error,
            "auto-battle rejected conduit group"
        )
    })
    .ok()?;
    let pre_pool = pool.runtime_view(managers);
    let score = executed_skills(&result.outcomes, device.uid, skills)
        .into_iter()
        .map(|(skill, spent)| {
            skill_score(
                device.uid,
                skill,
                spent,
                managers,
                &pre_pool,
                &sim_catalog,
                determinism,
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
