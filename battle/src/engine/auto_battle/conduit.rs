use std::collections::{HashMap, HashSet};

use sonettobuf::FightDeviceOper;

use crate::engine::{
    manager::BattleManagers,
    mechanic::card::CardMechanic,
    runtime::determinism::RoundDeterminism,
    skill::{
        behavior::classify::BehaviorKind,
        condition::parse::ParsedConditionKind,
        effect::{SkillEffectCatalog, catalog::SkillEffectTag},
        target::TargetPool,
    },
};

use super::{Support, certain_buffs, is_shield_buff, support_need, target_options};

pub(super) type GroupScore = (i32, i32, i32, i32, i64);

pub(super) fn operation_eligible(
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    operation: FightDeviceOper,
) -> bool {
    let Some((source_uid, group)) = operation.uid.zip(operation.index) else {
        return false;
    };
    let Some(source) = pool.entity(source_uid) else {
        return false;
    };
    let Some(skills) = managers
        .conduit
        .devices()
        .find(|device| device.uid == source_uid)
        .and_then(|device| device.skill_groups.get(group.saturating_sub(1) as usize))
    else {
        return false;
    };
    !skills
        .iter()
        .any(|skill| catalog.is_big_skill(skill.skill_id))
        || CardMechanic.ultimate_resource_ready(managers, source)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn score_group(
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
    source_uid: i64,
    group: i32,
    planned_energy: &HashMap<(i64, i32), i32>,
    planned_stopped_skills: &HashSet<(i64, i32)>,
) -> GroupScore {
    let Some(source) = pool.entity(source_uid) else {
        return (0, 0, 0, 0, 0);
    };
    let Some(team) = pool.team_type(source_uid) else {
        return (0, 0, 0, 0, 0);
    };
    let Some(skills) = managers
        .conduit
        .devices()
        .find(|device| device.uid == source_uid)
        .and_then(|device| device.skill_groups.get(group.saturating_sub(1) as usize))
    else {
        return (0, 0, 0, 0, 0);
    };
    let mut selected = managers.clone();
    if selected
        .conduit
        .execute(crate::engine::manager::conduit::ConduitCommand::SelectGroup { source_uid, group })
        .is_err()
    {
        return (0, 0, 0, 0, 0);
    }
    let managers = &selected;
    let planned_power = |power_type| {
        // Live lookahead lends typed card energy team-wide but keeps pending generic energy with its owner.
        if power_type == 0 {
            return planned_energy
                .get(&(source_uid, power_type))
                .copied()
                .unwrap_or_default();
        }
        planned_energy
            .iter()
            .filter(|((uid, candidate_type), _)| {
                *candidate_type == power_type && pool.team_type(*uid) == Some(team)
            })
            .map(|(_, amount)| *amount)
            .sum::<i32>()
    };
    let mut generic = managers
        .conduit
        .power(team, 0)
        .saturating_add(planned_power(0));
    let mut specific = HashMap::new();
    let mut total = (0, 0, 0, 0, 0);
    let mut used_once = HashSet::new();
    loop {
        let mut ran = false;
        for skill in skills {
            if catalog.get(skill.skill_id).is_none()
                || !catalog.issues(skill.skill_id).is_empty()
                || skill.is_stopped
                || planned_stopped_skills.contains(&(source_uid, skill.skill_id))
                || used_once.contains(&skill.skill_id)
                || super::card_conduit_switch(source_uid, skill.skill_id, managers, pool, catalog)
                    .is_some_and(|redirected| redirected != group)
                || (catalog.is_attack(skill.skill_id)
                    && target_options(
                        source_uid,
                        skill.skill_id,
                        None,
                        managers,
                        pool,
                        catalog,
                        determinism,
                    )
                    .is_empty())
            {
                continue;
            }
            let reduction = crate::engine::skill::buff_act::device_cost_reduce::modifier(
                managers,
                source_uid,
                skill.skill_id,
            )
            .map(|(reduction, _)| reduction)
            .unwrap_or_default();
            let spent = skill.cost_after_reduction(reduction);
            if skill.cost_type == 999 {
                if !CardMechanic.ultimate_resource_ready(managers, source) {
                    continue;
                }
                used_once.insert(skill.skill_id);
            } else {
                let available = specific.entry(skill.cost_type).or_insert_with(|| {
                    managers
                        .conduit
                        .power(team, skill.cost_type)
                        .saturating_add(planned_power(skill.cost_type))
                });
                if available.saturating_add(generic) < spent {
                    continue;
                }
                let specific_spent = spent.min(*available);
                *available -= specific_spent;
                generic -= spent - specific_spent;
                if spent == 0 {
                    used_once.insert(skill.skill_id);
                }
            }
            let score = skill_score(
                source_uid,
                skill.skill_id,
                spent,
                managers,
                pool,
                catalog,
                determinism,
            );
            total = (
                total.0 + score.0,
                total.1 + score.1,
                total.2 + score.2,
                total.3.max(score.3),
                total.4 + score.4,
            );
            ran = true;
        }
        if !ran {
            break;
        }
    }
    total
}

pub(super) fn selected_power_types(
    managers: &BattleManagers,
    operations: &[FightDeviceOper],
) -> HashMap<(i64, i32), bool> {
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
                .map(|skills| (uid, skills))
        })
        .flat_map(|(uid, skills)| skills.iter().map(move |skill| (uid, skill)))
        .filter(|(_, skill)| skill.cost_type > 0 && skill.cost_type != 999)
        .fold(HashMap::new(), |mut selected, (uid, skill)| {
            selected
                .entry((uid, skill.cost_type))
                .and_modify(|paid| *paid |= skill.cost_value > 0)
                .or_insert(skill.cost_value > 0);
            selected
        })
}

fn skill_score(
    source_uid: i64,
    skill_id: i32,
    spent: i32,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
) -> GroupScore {
    let Some(source) = pool.entity(source_uid) else {
        return (0, 0, 0, 0, 0);
    };
    let targets = target_options(
        source_uid,
        skill_id,
        None,
        managers,
        pool,
        catalog,
        determinism,
    );
    let buffs = if catalog.is_attack(skill_id) {
        Vec::new()
    } else {
        certain_buffs(skill_id, catalog)
    };
    let support = conduit_support_need(
        catalog.effect_tag(skill_id) == SkillEffectTag::Heal as i32,
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
    let (affinity, attack_scope, attack_score) = if catalog.is_attack(skill_id) {
        let attack_career = catalog
            .get(skill_id)
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
        let mut affinities = targets
            .iter()
            .filter_map(|uid| pool.entity(*uid))
            .map(|target| {
                managers
                    .catalog()
                    .career_multiplier(attack_career, target.career)
            })
            .map(i64::from)
            .collect::<Vec<_>>();
        affinities.sort_unstable_by(|left, right| right.cmp(left));
        let target_count = catalog
            .target_limit(skill_id)
            .max(
                managers
                    .catalog()
                    .target_count(catalog.logic_target(skill_id))
                    .max(0) as usize,
            )
            .max(1)
            .min(affinities.len());
        let affinity = affinities.first().copied().unwrap_or_default();
        let attack_score_affinity = if managers
            .catalog()
            .damage_target_count_kind(catalog.logic_target(skill_id))
            == 1
        {
            affinity
        } else {
            affinities.iter().take(target_count).sum()
        };
        (
            i32::try_from(affinity.saturating_sub(1_000)).unwrap_or(i32::MAX),
            i32::try_from(target_count).unwrap_or(i32::MAX),
            i64::from(catalog.damage_rate(skill_id)) * attack_score_affinity
                / 1_000
                / i64::from(spent.saturating_add(1)),
        )
    } else {
        (0, 0, 0)
    };
    (
        i32::from(catalog.is_big_skill(skill_id)),
        support_score,
        affinity,
        attack_scope,
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
