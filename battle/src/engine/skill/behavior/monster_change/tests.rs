use super::*;
use crate::engine::{
    manager::BattleManagers,
    runtime::determinism::RoundDeterminism,
    skill::{
        action::SkillModifiers,
        behavior::classify::BehaviorSpec,
        target::{TargetContext, TargetPool},
    },
};

#[test]
fn monster_change_preserves_configured_model_and_parameters() {
    let managers = BattleManagers::default();
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(40006, "MonsterChange"),
        vec![251407, 1000, 1],
        vec!["251407".into(), "1000".into(), "1".into()],
    );

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: -7,
            source_team: 2,
            target_uid: -7,
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
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::MonsterChange(
            MonsterChangeCommand {
                entity: EntityCommand {
                    source_uid: -7,
                    target_uid: -7,
                    operation: EntityOperation::Transform {
                        model_id: 251407,
                        parameters: [1000, 1],
                    },
                    ..
                },
            }
        ))]
    ));
}
