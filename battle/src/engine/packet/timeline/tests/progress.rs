use super::*;
use crate::engine::{
    event::bus::EventBus,
    manager::{
        BattleManagers,
        fight_progress::{FightProgressChange, FightProgressState, FightProgressValueChange},
    },
    runtime::{determinism::RoundDeterminism, executor::execute_rule_op},
    skill::{
        action::SkillModifiers,
        behavior::{BehaviorOpContext, rule_ops},
        effect::catalog::SkillEffectCatalog,
        target::{TargetContext, TargetPool},
    },
};
use sonettobuf::{Fight, FightEntityInfo, FightProgressInfo, FightTeam};

#[test]
fn progress_initialization_projects_max_before_value() {
    let effects = project_change_with_reduce_hp(
        &BattleChange::FightProgress(FightProgressChange {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(100015, "ProgressInitNew"),
            },
            state: FightProgressState {
                id: 8,
                max: 100,
                value: 0,
                show_id: 10,
                skill_id: 119381117,
            },
        }),
        true,
    )
    .unwrap();

    assert_eq!(effects.len(), 2);
    assert_eq!(
        effects[0].effect_type,
        Some(EffectType::Progressmaxchange as i32)
    );
    assert_eq!(effects[0].target_id, Some(8));
    assert_eq!(effects[0].buff_act_id, Some(8));
    assert_eq!(effects[0].effect_num, Some(100));
    assert_eq!(effects[0].effect_num1, Some(10));
    assert_eq!(effects[0].reserve_str.as_deref(), Some("119381117"));
    assert_eq!(
        effects[1].effect_type,
        Some(EffectType::Progresschange as i32)
    );
    assert_eq!(effects[1].target_id, Some(8));
    assert_eq!(effects[1].effect_num, Some(0));
}

#[test]
fn progress_value_change_projects_the_committed_delta_and_clamp_boundary() {
    let state = FightProgressState {
        id: 8,
        max: 100,
        value: 100,
        show_id: 10,
        skill_id: 119381117,
    };
    let [effect] = project_change_with_reduce_hp(
        &BattleChange::FightProgressValue(FightProgressValueChange {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(100016, "ProgressChangeNew"),
            },
            before: state,
            after: FightProgressState { value: 0, ..state },
            delta: -100,
        }),
        true,
    )
    .unwrap()
    .try_into()
    .unwrap();

    assert_eq!(effect.effect_type, Some(EffectType::Progresschange as i32));
    assert_eq!(effect.target_id, Some(8));
    assert_eq!(effect.buff_act_id, Some(8));
    assert_eq!(effect.effect_num, Some(-100));
    assert_eq!(effect.reserve_str.as_deref(), Some("0"));

    let [effect] = project_change_with_reduce_hp(
        &BattleChange::FightProgressValue(FightProgressValueChange {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(1148, "HurtToProgress"),
            },
            before: FightProgressState { value: 0, ..state },
            after: FightProgressState { value: 34, ..state },
            delta: 34,
        }),
        true,
    )
    .unwrap()
    .try_into()
    .unwrap();

    assert_eq!(effect.effect_num, Some(34));
    assert_eq!(effect.reserve_str.as_deref(), Some("100"));
}

#[test]
fn configured_progress_reset_commits_and_projects_its_signed_delta() {
    crate::test_support::init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(1),
                passive_skill: vec![119381119],
                ..Default::default()
            }],
            ..Default::default()
        }),
        progress_list: vec![FightProgressInfo {
            id: Some(8),
            max: Some(100),
            value: Some(100),
            show_id: Some(10),
            skill_id: Some(119381117),
        }],
        ..Default::default()
    };
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let behavior = catalog
        .get(119381119)
        .unwrap()
        .slots
        .iter()
        .find(|slot| slot.behavior.spec.key.opcode == 100016)
        .unwrap()
        .behavior
        .clone();
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    let [operation] = rule_ops(
        BehaviorOpContext {
            source_uid: -1,
            source_team: 2,
            target_uid: -1,
            active_skill_id: 119381119,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap()
    .try_into()
    .unwrap();
    let outcome = execute_rule_op(&mut managers, &mut EventBus::default(), operation).unwrap();
    let [change] = outcome.changes().try_into().unwrap();
    let [effect] = project_change_with_reduce_hp(&change, true)
        .unwrap()
        .try_into()
        .unwrap();

    assert_eq!(managers.fight_progress.get(8).unwrap().value, 0);
    assert_eq!(effect.effect_type, Some(EffectType::Progresschange as i32));
    assert_eq!(effect.target_id, Some(8));
    assert_eq!(effect.buff_act_id, Some(8));
    assert_eq!(effect.effect_num, Some(-100));
}
