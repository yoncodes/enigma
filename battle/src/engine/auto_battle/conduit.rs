use std::collections::HashSet;

use sonettobuf::{BeginRoundOper, FightDeviceOper};

use crate::engine::{
    manager::{BattleManagers, conduit::ConduitChange},
    mechanic::card::CardMechanic,
    round::command::RoundCommand,
    runtime::{
        determinism::RoundDeterminism,
        drain,
        executor::RuleOutcome,
        schedule::{run_conduit_phase, run_player_phase},
    },
    skill::{
        behavior::classify::BehaviorKind,
        condition::parse::ParsedConditionKind,
        effect::{SkillEffectCatalog, catalog::SkillEffectTag},
        target::TargetPool,
    },
};

use super::{
    AutoBattleError, Support, certain_buffs, is_shield_buff, support_need, target_options,
};

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
pub(super) fn prepare_plan(
    fight: &sonettobuf::Fight,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
    context: crate::engine::skill::target::TargetContext,
    commands: &[BeginRoundOper],
    operations: &[FightDeviceOper],
) -> Result<PreparedPlan, AutoBattleError> {
    let mut simulated = managers.clone();
    simulated.begin_action_round();
    let simulated_catalog = catalog.clone();
    let mut simulated_determinism = determinism.clone();
    let selected = drain::run_command_group(
        &mut simulated,
        pool,
        &simulated_catalog,
        &mut simulated_determinism,
        context,
        operations.iter().filter_map(|operation| {
            Some(crate::engine::skill::rule::output::RuleOp::Command(
                crate::engine::skill::rule::output::BattleCommand::Conduit(
                    crate::engine::manager::conduit::ConduitCommand::SelectGroup {
                        source_uid: operation.uid?,
                        group: operation.index?,
                    },
                ),
            ))
        }),
    )?;
    let commands = commands
        .iter()
        .filter_map(RoundCommand::from_oper)
        .collect::<Vec<_>>();
    run_player_phase(
        fight,
        &mut simulated,
        pool,
        &simulated_catalog,
        &mut simulated_determinism,
        context,
        commands,
        selected.frames,
        1,
        crate::engine::manager::emitter::UID,
    )?;
    Ok(PreparedPlan {
        managers: simulated,
        catalog: simulated_catalog,
        determinism: simulated_determinism,
    })
}

pub(super) struct PreparedPlan {
    managers: BattleManagers,
    catalog: SkillEffectCatalog,
    determinism: RoundDeterminism,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn score_plan(
    prepared: &PreparedPlan,
    fight: &sonettobuf::Fight,
    pool: &TargetPool,
    context: crate::engine::skill::target::TargetContext,
    operations: &[FightDeviceOper],
    scored_uid: i64,
) -> Result<GroupScore, AutoBattleError> {
    let mut simulated = prepared.managers.clone();
    let mut simulated_catalog = prepared.catalog.clone();
    let mut simulated_determinism = prepared.determinism.clone();
    let scoring_managers = simulated.clone();
    let scoring_pool = pool.runtime_view(&scoring_managers);
    let result = run_conduit_phase(
        simulated.catalog(),
        fight,
        &mut simulated,
        pool,
        &mut simulated_catalog,
        &mut simulated_determinism,
        context,
        operations,
    )?;
    Ok(result
        .outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            RuleOutcome::Conduit(ConduitChange::SkillBegan {
                source_uid,
                skill_id,
                spent,
                ..
            }) if *source_uid == scored_uid => Some((*source_uid, *skill_id, *spent)),
            _ => None,
        })
        .map(|(source_uid, skill_id, spent)| {
            skill_score(
                source_uid,
                skill_id,
                spent,
                &scoring_managers,
                &scoring_pool,
                &simulated_catalog,
                &prepared.determinism,
            )
        })
        .fold((0, 0, 0, 0, 0), |total, score| {
            (
                total.0 + score.0,
                total.1 + score.1,
                total.2 + score.2,
                total.3.max(score.3),
                total.4 + score.4,
            )
        }))
}

pub(super) fn selected_power_types(
    managers: &BattleManagers,
    operations: &[FightDeviceOper],
) -> HashSet<(i64, i32)> {
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
        .filter(|(_, skill)| skill.cost_type > 0 && skill.cost_type != 999 && skill.cost_value > 0)
        .map(|(uid, skill)| (uid, skill.cost_type))
        .collect()
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
