use crate::engine::{
    manager::{
        BattleManagers,
        buff::BuffCommand,
        gauge::{GaugeCommand, GaugeOperation},
    },
    runtime::determinism::RoundDeterminism,
    skill::{
        action::SkillModifiers,
        behavior::{BehaviorOpContext, classify::BehaviorSpec},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
        target::{TargetContext, TargetPool},
    },
};

fn shared_pool_value_change_ops(managers: &BattleManagers, args: Vec<i32>) -> Vec<RuleOp> {
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60191, "BloodPoolValueChange"),
        args,
        Vec::new(),
    );
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();

    super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 20,
            active_skill_id: 108280012,
            transfer_count: 1,
            event: None,
            managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .expect("registered BloodPoolValueChange must emit")
}

#[test]
fn parses_consume_blood_add_buff_rule_behavior() {
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60211, "ConsumeBloodAddBuff2"),
        vec![4, 31200143, 4],
        Vec::new(),
    );

    assert_eq!(
        super::ConsumeBloodAddBuff::from_behavior(&behavior),
        Some(super::ConsumeBloodAddBuff {
            cost: 4,
            buff_id: 31200143,
            count: 4,
        })
    );
    assert_eq!(super::references(&behavior).buffs, vec![31200143]);
}

#[test]
fn count_argument_only_sets_supported_repeat_amount() {
    crate::test_support::init_config();

    assert_eq!(
        super::ConsumeBloodAddBuff {
            cost: 4,
            buff_id: 31200143,
            count: 4,
        }
        .grant_amount(),
        None
    );
    assert_eq!(
        super::ConsumeBloodAddBuff {
            cost: 1,
            buff_id: 31260151,
            count: 6,
        }
        .grant_amount(),
        Some(6)
    );
}

#[test]
fn consume_blood_add_buff_emits_one_atomic_command() {
    crate::test_support::init_config();

    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60210, "ConsumeBloodAddBuff"),
        vec![1, 31260151, 6],
        Vec::new(),
    );
    let definition = super::super::registry::find(&behavior).unwrap();
    assert_eq!(
        definition.output_owner,
        super::super::registry::OutputOwner::Skill
    );
    let key = crate::engine::mechanic::bloodtithe::rule::key(1);
    let managers = BattleManagers::default();
    let emit = |managers: &BattleManagers| {
        let pool = TargetPool::default();
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();

        super::super::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 20,
                active_skill_id: 31260201,
                transfer_count: 1,
                event: None,
                managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .unwrap()
    };

    let ops = emit(&managers);
    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::BloodtitheSpend(command))]
            if command.gauge.key == key
            && command.gauge.operation == GaugeOperation::ChangeValue { delta: -1 }
            && matches!(
                &command.buff,
                BuffCommand::Grant(grant)
                    if grant.source_uid == 10
                        && grant.target_uid == 20
                        && grant.buff_id == 31260151
                        && grant.amount == Some(6)
            )
    ));
}

#[test]
fn lingering_glow_value_change_is_a_parent_owned_destination() {
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60191, "BloodPoolValueChange"),
        vec![20_000, 1],
        Vec::new(),
    );
    let definition = super::super::registry::find(&behavior).unwrap();

    assert!(definition.destination);
    assert_eq!(
        definition.output_owner,
        super::super::registry::OutputOwner::Parent
    );
    assert_eq!(
        super::SharedPoolMutation::from_behavior(&behavior, 1),
        Some(super::SharedPoolMutation {
            key: crate::engine::mechanic::lingering_glow::key(1),
            operation: crate::engine::manager::gauge::GaugeOperation::ChangeValue { delta: 20 },
            raw_delta: Some(20_000),
            progress_raw_delta: Some(20_000),
        })
    );
}

#[test]
fn ordinary_blood_pool_value_keeps_its_unscaled_parent_owned_points() {
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60191, "BloodPoolValueChange"),
        vec![20],
        Vec::new(),
    );

    assert_eq!(
        super::SharedPoolMutation::from_behavior(&behavior, 2),
        Some(super::SharedPoolMutation {
            key: crate::engine::mechanic::bloodtithe::rule::key(2),
            operation: crate::engine::manager::gauge::GaugeOperation::ChangeValue { delta: 20 },
            raw_delta: None,
            progress_raw_delta: None,
        })
    );
    let definition = super::super::registry::find(&behavior).unwrap();
    assert_eq!(
        (definition.output_owner_for)(&behavior, &RuleOp::FreezeActiveSkillRates, 0),
        None
    );
    assert_eq!(
        definition.output_owner,
        super::super::registry::OutputOwner::Parent
    );
}

#[test]
fn blood_pool_value_change_without_bloodtithe_gauge_emits_no_operation() {
    let managers = BattleManagers::default();

    assert!(shared_pool_value_change_ops(&managers, vec![4]).is_empty());
}

#[test]
fn lingering_glow_value_change_without_gauge_emits_no_operation() {
    let managers = BattleManagers::default();

    assert!(shared_pool_value_change_ops(&managers, vec![5_000, 1]).is_empty());
}

#[test]
fn blood_pool_value_change_with_bloodtithe_gauge_emits_configured_delta() {
    let mut managers = BattleManagers::default();
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60191, "BloodPoolValueChange"),
        vec![4],
        Vec::new(),
    );
    let key = crate::engine::mechanic::bloodtithe::rule::key(1);
    managers
        .execute_gauge(GaugeCommand::new(
            super::super::command_origin(&behavior).unwrap(),
            key,
            GaugeOperation::Enable { max: Some(100) },
        ))
        .unwrap();

    let ops = shared_pool_value_change_ops(&managers, vec![4]);
    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Gauge(command))]
            if command.key == key
                && command.operation == GaugeOperation::ChangeValue { delta: 4 }
    ));
}
