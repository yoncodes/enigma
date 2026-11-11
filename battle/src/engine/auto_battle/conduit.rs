use std::collections::HashSet;

use sonettobuf::FightDeviceOper;

use crate::engine::{
    manager::BattleManagers,
    runtime::determinism::RoundDeterminism,
    skill::{
        effect::{SkillEffectCatalog, catalog::SkillEffectTag},
        target::TargetPool,
    },
};

use super::{Support, certain_buffs, is_shield_buff, support_need, target_options};

mod selection;

pub(super) use selection::select_groups;

pub(super) type GroupScore = (i32, i32, i32, i32, i64);

pub(super) fn selected_power_types(
    managers: &BattleManagers,
    operations: &[FightDeviceOper],
) -> HashSet<(i32, i32)> {
    operations
        .iter()
        .filter_map(|operation| Some((operation.uid?, operation.index?)))
        .filter_map(|(uid, group)| {
            let team = managers.entity.team_type(uid)?;
            managers
                .conduit
                .devices()
                .find(|device| device.uid == uid)?
                .skill_groups
                .get(group.saturating_sub(1) as usize)
                .map(|skills| (team, skills))
        })
        .flat_map(|(team, skills)| skills.iter().map(move |skill| (team, skill)))
        .filter(|(_, skill)| skill.cost_type > 0 && skill.cost_type != 999 && skill.cost_value > 0)
        .map(|(team, skill)| (team, skill.cost_type))
        .collect()
}

pub(super) fn add_score(left: GroupScore, right: GroupScore) -> GroupScore {
    (
        left.0 + right.0,
        left.1 + right.1,
        left.2 + right.2,
        left.3.max(right.3),
        left.4 + right.4,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn skill_score(
    source_uid: i64,
    skill_id: i32,
    spent: i32,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
    context: crate::engine::skill::target::TargetContext,
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
        let attack_career = targets
            .first()
            .map(|target_uid| {
                crate::engine::skill::behavior::rate::active_modifiers(
                    source_uid,
                    skill_id,
                    *target_uid,
                    crate::engine::skill::behavior::rate::RateRuntime {
                        effects: catalog,
                        managers,
                        pool,
                        context,
                    },
                    determinism,
                )
                .attack_career
                .unwrap_or(source.career)
            })
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
