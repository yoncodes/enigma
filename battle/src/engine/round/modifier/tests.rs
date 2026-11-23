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

#[test]
fn active_buff_action_modifier_applies_to_its_own_living_team() {
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
    let origin = crate::engine::skill::rule::CommandOrigin {
        domain: crate::engine::skill::rule::RuleDomain::Behavior,
        key: crate::engine::skill::rule::DefinitionKey::new(1, "AddBuff"),
    };
    managers
        .execute_buff(crate::engine::manager::buff::BuffCommand::Grant(
            crate::engine::manager::buff::BuffGrant {
                origin,
                source_uid: -1,
                target_uid: -1,
                buff_id: 2_137_001,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))
        .unwrap();

    assert_eq!(active_buff_action_bonus(&managers, 1), 0);
    assert_eq!(active_buff_action_bonus(&managers, 2), 1);
    assert_eq!(
        ai_action_bonus(&pool, &managers, &catalog, &mut determinism, context),
        1
    );

    let mut dead_fight = fight;
    dead_fight.defender.as_mut().unwrap().entitys[0].current_hp = Some(0);
    let mut dead_managers = BattleManagers::seeded(&dead_fight);
    dead_managers
        .execute_buff(crate::engine::manager::buff::BuffCommand::Grant(
            crate::engine::manager::buff::BuffGrant {
                origin,
                source_uid: -1,
                target_uid: -1,
                buff_id: 2_137_001,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))
        .unwrap();

    assert_eq!(active_buff_action_bonus(&dead_managers, 2), 0);
}
