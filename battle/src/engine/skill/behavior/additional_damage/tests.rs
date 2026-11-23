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
fn configured_additional_damage_is_cast_local_and_grants_its_data_buff() {
    crate::test_support::init_config();
    let managers = BattleManagers::default();
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60206, "CreateAdditionalDamageAddBuff"),
        vec![1, 1000, 2, 31200111],
        Vec::new(),
    );

    let ops = behavior::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31200114,
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
        [RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
            BuffGrant {
                source_uid: 10,
                target_uid: 10,
                buff_id: 31200111,
                ..
            }
        )))]
    ));
    assert_eq!(
        modifiers.additional_damage,
        vec![AdditionalDamageModifier {
            origin: super::super::command_origin(&behavior).unwrap(),
            buff_id: 31200111,
        }]
    );
}
