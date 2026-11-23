use crate::engine::{
    manager::BattleManagers,
    runtime::determinism::RoundDeterminism,
    skill::{
        condition::{conditions_fire_count, satisfied_conditions},
        effect::SkillEffectCatalog,
        subscriber,
        target::{TargetContext, TargetPool, TargetResolver},
    },
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RoundModifiers {
    pub action_points: i32,
}

pub fn action_point_bonus(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
) -> i32 {
    action_bonus_for_team(pool, managers, catalog, determinism, context, 1)
}

pub fn ai_action_bonus(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
) -> i32 {
    active_buff_action_bonus(managers, 2)
        + action_bonus_for_team(pool, managers, catalog, determinism, context, 2)
}

pub(crate) fn active_buff_action_bonus(managers: &BattleManagers, team_type: i32) -> i32 {
    managers
        .buff
        .active_features(&managers.hp)
        .iter()
        .filter(|feature| feature.team_type == team_type)
        .map(crate::engine::skill::buff_act::add_action_point::bonus)
        .sum()
}

fn action_bonus_for_team(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    team_type: i32,
) -> i32 {
    subscriber::active_skills(pool, managers)
        .into_iter()
        .flat_map(|(owner_uid, skill_id)| {
            catalog.get(skill_id).into_iter().flat_map(move |effect| {
                effect
                    .slots
                    .iter()
                    .map(move |slot| (owner_uid, skill_id, slot))
            })
        })
        .filter_map(|(owner_uid, skill_id, slot)| {
            let definition = crate::engine::skill::behavior::registry::find(&slot.behavior)?;
            let collect = definition.collect_round_modifier?;
            let route = slot.compiled_route.as_ref().ok()?;
            let condition_targets = TargetResolver::resolve_with_managers_and_context(
                &slot.condition_target,
                skill_id,
                owner_uid,
                pool,
                determinism,
                Some(managers),
                context,
            );
            let repeats = route
                .branches
                .iter()
                .filter_map(|branch| match branch.driver {
                    None => Some(slot.conditions.clone()),
                    Some(crate::engine::skill::rule::route::ConditionDriver::Setup(setup))
                        if matches!(
                            setup.stage,
                            crate::engine::skill::rule::SetupStage::EnterFight
                                | crate::engine::skill::rule::SetupStage::RoundStart
                                | crate::engine::skill::rule::SetupStage::RoundStartCondition
                        ) =>
                    {
                        Some(satisfied_conditions(&slot.conditions, setup.key))
                    }
                    _ => None,
                })
                .map(|conditions| {
                    conditions_fire_count(
                        &conditions,
                        owner_uid,
                        &condition_targets,
                        Some(managers),
                        pool,
                        context,
                    )
                })
                .max()
                .unwrap_or_default();
            if repeats <= 0 {
                return None;
            }
            let targets = TargetResolver::resolve_with_managers_and_context(
                &slot.target,
                skill_id,
                owner_uid,
                pool,
                determinism,
                Some(managers),
                context,
            );
            targets
                .iter()
                .any(|uid| pool.team_type(*uid) == Some(team_type))
                .then(|| collect(&slot.behavior))
                .flatten()
                .map(|modifier| modifier.action_points.saturating_mul(repeats))
        })
        .sum()
}

#[cfg(test)]
mod tests;
