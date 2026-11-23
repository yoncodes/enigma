use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::{
    manager::BattleManagers,
    runtime::determinism::RoundDeterminism,
    skill::{
        action::SkillModifiers,
        behavior::{self, classify::BehaviorSpec},
        target::{TargetContext, TargetPool},
    },
};

#[test]
fn shell_assign_moves_the_configured_amount_from_stock_to_target() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                buffs: vec![BuffInfo {
                    uid: Some(52),
                    buff_id: Some(31090111),
                    layer: Some(16),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60133, "ShellAssign"),
        vec![3, 31090111],
        Vec::new(),
    );
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();

    let ops = behavior::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 31090111,
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
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Shell(
            ShellCommand::Deploy {
                source_uid: 10,
                target_uid: -1,
                stock_buff_id: 31090111,
                amount: 3,
                ..
            }
        ))]
    ));
}

#[test]
fn shell_use_skill_counts_one_deployment_or_retrieval_per_change() {
    let managers = BattleManagers::default();
    let pool = TargetPool::default();
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60135, "ShellUseSkill"),
        vec![5, 31090174],
        Vec::new(),
    );
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext {
        shell_change_amount: 3,
        ..Default::default()
    };

    let ops = behavior::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 31090196,
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
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Shell(
            ShellCommand::AccumulateAndUseSkill {
                source_uid: 10,
                target_uid: -1,
                threshold: 5,
                delta: 1,
                skill_id: 31090174,
                ..
            }
        ))]
    ));
}
