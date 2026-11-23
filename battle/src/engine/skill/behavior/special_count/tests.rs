use super::*;
use crate::engine::{
    manager::BattleManagers,
    skill::{
        action::SkillModifiers,
        rule::output::BattleCommand,
        target::{TargetContext, TargetPool},
    },
};
use crate::test_support::init_config;
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

#[test]
fn descriptor_reports_added_buff() {
    let behavior = ParsedBehavior::new(60205, "AddBuffAndAddSpecialCount", vec![101, 1, 1, 102]);

    assert_eq!(references(&behavior).buffs, vec![101]);
}

#[test]
fn special_count_rate_is_emitted_as_a_skill_modifier() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(90071),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.buff.add_special_count(10, &[90071], 3);
    let pool = TargetPool::default();
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    let parsed = ParsedBehavior::from_spec(
        ParsedBehavior::new(60202, "AddSkillRateBySpecialCount", vec![250, 90071]).spec,
        vec![250, 90071],
        vec!["250".into(), "90071".into()],
    );

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 123,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &parsed,
    );

    assert_eq!(ops, Some(Vec::new()));
    assert_eq!(modifiers.rates.len(), 1);
    assert_eq!(modifiers.rates[0].fixed_value(), Some(750));
    assert_eq!(modifiers.rates[0].target_uid, 0);
}

#[test]
fn combined_special_count_scales_configured_values_by_transfer_count() {
    init_config();
    let managers = BattleManagers::default();
    let pool = TargetPool::default();
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    let behavior = ParsedBehavior::new(
        60205,
        "AddBuffAndAddSpecialCount",
        vec![90071, 5, 1, 31070111],
    );

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 123,
            transfer_count: 3,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert!(matches!(
        &ops[0],
        RuleOp::Command(BattleCommand::Buff(
            crate::engine::manager::buff::BuffCommand::Grant(grant)
        )) if grant.amount == Some(15)
    ));
    assert!(matches!(
        &ops[1],
        RuleOp::Command(BattleCommand::Buff(
            crate::engine::manager::buff::BuffCommand::AddSpecialCount(change)
        )) if change.count == 3
    ));
}

#[test]
fn field_transfer_count_is_resolved_through_the_registered_behavior() {
    let mut managers = BattleManagers::default();
    managers.field.record_transfer(1);
    managers.field.record_transfer(1);
    let behavior = ParsedBehavior::new(60205, "AddBuffAndAddSpecialCount", vec![435021, 1, 1, -1]);
    let definition = super::super::registry::find(&behavior).unwrap();
    assert_eq!(
        (definition.resolve_fire_count)(
            super::super::registry::BehaviorFireCountContext {
                managers: &managers,
                source_team: 1,
            },
            &behavior,
            1,
        ),
        2
    );
}

#[test]
fn configured_special_count_transfer_unit_is_not_collapsed_to_one() {
    let managers = BattleManagers::default();
    let pool = TargetPool::default();
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    let behavior = ParsedBehavior::new(60204, "AddBuffSpecialCount", vec![5, 31070111]);

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 123,
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
    .unwrap();

    assert!(matches!(
        &ops[0],
        RuleOp::Command(BattleCommand::Buff(
            crate::engine::manager::buff::BuffCommand::AddSpecialCount(change)
        )) if change.count == 5
    ));
}
