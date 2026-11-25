use super::*;
use crate::engine::{
    manager::card::{CardCommand, CardEnergyChange},
    skill::rule::output::BattleCommand,
};

#[test]
fn exact_definition_owns_its_destination_op_emitter() {
    let managers = BattleManagers::default();
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    let behavior = ParsedBehavior::new(60189, "AddEnergyToCard", vec![1, -1, 3]);

    assert!(matches!(
        rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 0,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        ),
        Some(ops) if matches!(ops.as_slice(), [RuleOp::Command(BattleCommand::Card(
            CardCommand::ChangeBasicEnergy(CardEnergyChange {
                delta: 3,
                count: 1,
                ..
            })
        ))])
    ));
}

#[test]
fn legacy_progress_initializer_requires_three_raw_arguments() {
    let mut behavior = ParsedBehavior::new(60106, "ProgressInit", vec![0, 100, 2]);
    assert!(is_supported(&behavior));

    behavior.raw_arity = 4;
    assert!(!is_supported(&behavior));
}

#[test]
fn progress_init_new_emits_the_exact_progress_definition() {
    let managers = BattleManagers::default();
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    let behavior = ParsedBehavior::new(100015, "ProgressInitNew", vec![0, 100, 10, 8]);

    assert!(matches!(
        rule_ops(
            BehaviorOpContext {
                source_uid: -99999,
                source_team: 2,
                target_uid: -99999,
                active_skill_id: 119381117,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        ),
        Some(ops) if matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::FightProgress(command))]
                if command.initial == 0
                    && command.max == 100
                    && command.show_id == 10
                    && command.progress_id == 8
                    && command.skill_id == 119381117
        )
    ));
}

#[test]
fn progress_change_new_emits_the_signed_delta() {
    let managers = BattleManagers::default();
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    let behavior = ParsedBehavior::new(100016, "ProgressChangeNew", vec![1, -100, 8]);

    assert!(matches!(
        rule_ops(
            BehaviorOpContext {
                source_uid: -99999,
                source_team: 2,
                target_uid: -99999,
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
        ),
        Some(ops) if matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::FightProgressValue(command))]
                if command.delta == -100 && command.progress_id == 8
        )
    ));
}

#[test]
fn exact_ultimate_kind_definition_emits_entity_owned_state() {
    crate::test_support::init_config();
    let fight = sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                ex_skill: Some(900),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    let behavior = ParsedBehavior::new(100012, "EzioBigSkillWeapon2", Vec::new());

    assert!(matches!(
        rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 0,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        ),
        Some(ops) if matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::EntitySkill(command))]
                if command.target_uid == 10
                    && command.ultimate_kind
                        == crate::engine::skill::condition::extra::ExtraSkillKind::ExtraAction
                    && command.origin.key
                        == crate::engine::skill::rule::DefinitionKey::new(
                            100012,
                            "EzioBigSkillWeapon2",
                        )
        )
    ));
    assert!(registry::find_key(100012, "EzioBigSkillWeapon2").is_some());
    assert!(registry::find_key(100012, "EzioReuse").is_none());
}
