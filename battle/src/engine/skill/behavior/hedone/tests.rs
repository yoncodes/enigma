use crate::engine::{
    manager::BattleManagers,
    runtime::determinism::RoundDeterminism,
    skill::{
        action::SkillModifiers,
        behavior::{BehaviorOpContext, classify::BehaviorSpec, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
        target::{TargetContext, TargetPool},
    },
};

use super::*;

#[test]
fn emits_the_exact_stored_damage_command() {
    let managers = BattleManagers::default();
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60332, "HedoneHurt"),
        vec![500],
        vec!["500".into()],
    );
    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: -4,
            source_team: 1,
            target_uid: 2,
            active_skill_id: 31560133,
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
        [RuleOp::Command(BattleCommand::HedoneDamage(
            HedoneDamageCommand {
                source_uid: -4,
                target_uid: 2,
                rate: 500,
                config_effect: 60332,
                skill_id: 31560133,
                ..
            }
        ))]
    ));
}

#[test]
fn requires_one_positive_rate() {
    for args in [vec![1000], vec![500]] {
        assert!(Handler::supports(&ParsedBehavior::new(
            60332,
            "HedoneHurt",
            args
        )));
    }
    for args in [vec![], vec![0], vec![-1], vec![500, 1]] {
        assert!(!Handler::supports(&ParsedBehavior::new(
            60332,
            "HedoneHurt",
            args
        )));
    }
}
