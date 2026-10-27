use std::{cmp::Reverse, collections::HashSet};

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

use super::{Support, certain_buffs, support_need, target_options};

pub(super) mod lookahead;
#[cfg(test)]
mod tests;

type GroupScore = (i32, i32, i64);

pub(super) fn operations(
    fight: &Fight,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
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

        let index = best
            .map(|(.., Reverse(index))| index as i32 + 1)
            .unwrap_or(device.selected_group);
        operations.push(FightDeviceOper {
            uid: Some(device.uid),
            index: Some(index),
        });
    }

    operations
}

pub(super) fn selection_operations(
    fight: &Fight,
    round_start: &BattleManagers,
    after_cards: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
) -> Vec<FightDeviceOper> {
    let mut operations = operations(fight, after_cards, pool, catalog, determinism);
    // Card-driven group changes run after devicesOpers, so do not echo the same change early.
    for operation in &mut operations {
        let Some(uid) = operation.uid else { continue };
        let Some(round_start_group) = round_start.conduit.selected_group(uid) else {
            continue;
        };
        let Some(after_cards_group) = after_cards.conduit.selected_group(uid) else {
            continue;
        };
        if round_start_group != after_cards_group && operation.index == Some(after_cards_group) {
            operation.index = Some(round_start_group);
        }
    }
    operations
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
    let support = support_need(
        catalog.effect_tag(skill.skill_id) == SkillEffectTag::Heal as i32,
        &buffs,
        &targets,
        managers,
        pool,
    );
    let support_score = match support {
        Support::Urgent => 2,
        Support::Needed => 1,
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
