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
    action_bonus_for_team(pool, managers, catalog, determinism, context, 2)
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
mod tests {
    use sonettobuf::{Fight, FightEntityInfo, FightTeam};

    use super::*;
    use crate::test_support::init_config;

    fn battle_nine_fight() -> Fight {
        Fight {
            battle_id: Some(9_001_101),
            episode_id: Some(90_400_101),
            cur_round: Some(5),
            attacker: Some(FightTeam {
                entitys: (1..=4)
                    .map(|uid| FightEntityInfo {
                        uid: Some(uid),
                        current_hp: Some(1_000),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(1_000),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn battle_rule_add_act_reads_the_active_buff_condition() {
        init_config();
        let fight = battle_nine_fight();
        let pool = TargetPool::from_fight(&fight);
        let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
        let mut managers = BattleManagers::seeded(&fight);
        let mut determinism = RoundDeterminism::default();
        let context = TargetContext {
            battle_id: fight.battle_id.unwrap(),
            current_round: fight.cur_round.unwrap(),
            ..Default::default()
        };

        assert_eq!(
            action_point_bonus(&pool, &managers, &catalog, &mut determinism, context),
            0
        );

        let origin = crate::engine::skill::rule::CommandOrigin {
            domain: crate::engine::skill::rule::RuleDomain::Behavior,
            key: crate::engine::skill::rule::DefinitionKey::new(1, "AddBuff"),
        };
        managers
            .execute_buff(crate::engine::manager::buff::BuffCommand::Grant(
                crate::engine::manager::buff::BuffGrant {
                    origin,
                    source_uid: 1,
                    target_uid: 1,
                    buff_id: 23_390_031,
                    amount: None,
                    occurrences: 1,
                    child_uid_reservations: 0,
                },
            ))
            .unwrap();
        assert_eq!(
            action_point_bonus(&pool, &managers, &catalog, &mut determinism, context),
            1
        );

        managers
            .execute_buff(crate::engine::manager::buff::BuffCommand::Remove(
                crate::engine::manager::buff::BuffRemove {
                    origin,
                    target_uid: 1,
                    selector: crate::engine::manager::buff::BuffRemoveSelector::ExactId(23_390_031),
                },
            ))
            .unwrap();
        managers
            .execute_buff(crate::engine::manager::buff::BuffCommand::Grant(
                crate::engine::manager::buff::BuffGrant {
                    origin,
                    source_uid: 1,
                    target_uid: 1,
                    buff_id: 23_390_041,
                    amount: None,
                    occurrences: 1,
                    child_uid_reservations: 0,
                },
            ))
            .unwrap();
        assert_eq!(
            action_point_bonus(&pool, &managers, &catalog, &mut determinism, context),
            -1
        );
    }

    #[test]
    fn enter_fight_action_modifier_applies_to_its_own_team() {
        init_config();
        let mut fight = battle_nine_fight();
        fight.defender.as_mut().unwrap().entitys[0].passive_skill = vec![2301];
        let pool = TargetPool::from_fight(&fight);
        let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
        assert!(
            catalog.get(2301).unwrap().slots[0]
                .compiled_setup_keys(crate::engine::skill::rule::SetupStage::EnterFight, 0)
                .unwrap()
                .is_empty()
        );
        let managers = BattleManagers::seeded(&fight);
        let mut determinism = RoundDeterminism::default();
        let context = TargetContext {
            battle_id: fight.battle_id.unwrap(),
            current_round: fight.cur_round.unwrap(),
            ..Default::default()
        };

        assert_eq!(
            action_point_bonus(&pool, &managers, &catalog, &mut determinism, context),
            0
        );
        assert_eq!(
            ai_action_bonus(&pool, &managers, &catalog, &mut determinism, context),
            1
        );
    }
}
