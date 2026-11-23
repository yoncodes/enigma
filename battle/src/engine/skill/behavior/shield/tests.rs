use super::*;
use crate::engine::{
    manager::BattleManagers,
    runtime::determinism::RoundDeterminism,
    skill::{
        action::SkillModifiers,
        target::{TargetContext, TargetPool},
    },
};

fn registered_carrier_uid(behavior: &ParsedBehavior) -> ShieldCarrierUid {
    let managers = BattleManagers::default();
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    let ops = super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 11,
            active_skill_id: 1,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        behavior,
    )
    .unwrap();
    match ops.as_slice() {
        [RuleOp::Command(BattleCommand::Shield(command))] => command.carrier_uid,
        _ => panic!("expected one shield command"),
    }
}

#[test]
fn exact_supply_shield_routes_keep_distinct_carrier_uid_policies() {
    let kiperina = ParsedBehavior::new(
        60183,
        "SupplyShield2",
        vec![31170002, 102, 1500, 0, 102, 6500],
    );
    let marsha = ParsedBehavior::new(
        60259,
        "SupplyShield2",
        vec![31270012, 102, 1800, 0, 102, 6500, 201, 900],
    );

    assert_eq!(registered_carrier_uid(&kiperina), ShieldCarrierUid::Child);
    assert_eq!(
        registered_carrier_uid(&marsha),
        ShieldCarrierUid::Definition
    );
}

#[test]
fn supply_shield_uses_its_configured_buff_stack_rate() {
    crate::test_support::init_config();
    let managers = BattleManagers::seeded(&sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![
                sonettobuf::FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1_000),
                    attr: Some(sonettobuf::HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(2_028),
                        ..Default::default()
                    }),
                    buffs: vec![sonettobuf::BuffInfo {
                        uid: Some(20),
                        buff_id: Some(4_150_001),
                        from_uid: Some(10),
                        layer: Some(3),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                sonettobuf::FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(1_000),
                    attr: Some(sonettobuf::HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    });
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    let behavior = ParsedBehavior::new(
        60183,
        "SupplyShield2",
        vec![
            30_941_121, 102, 1_500, 0, 102, 6_000, 102, 0, 4_150_001, 50, -1,
        ],
    );

    let ops = super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 11,
            active_skill_id: 30_941_124,
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
        [RuleOp::Command(BattleCommand::Shield(command))]
            if command.amount_rate == 1_650
                && command.max_rate == 6_000
                && command.multiplier_bonus == Some((AttrId::Attack, 0))
    ));
    assert_eq!(
        ChildUidHandler::references(&behavior).buffs,
        [30_941_121, 4_150_001]
    );
    assert!(ChildUidHandler::supports(&behavior));
    assert!(!Handler::supports(&behavior));
    assert!(!ChildUidHandler::supports(&ParsedBehavior::new(
        60183,
        "SupplyShield2",
        vec![
            30_941_121, 102, 1_500, 1, 102, 6_000, 102, 0, 4_150_001, 50, -1,
        ],
    )));
    assert!(!ChildUidHandler::supports(&ParsedBehavior::new(
        60183,
        "SupplyShield2",
        vec![
            30_941_121, 102, 1_500, 0, 102, 6_000, 102, 0, 4_150_001, 50, 1,
        ],
    )));
}

#[test]
fn marsha_shield_emits_its_critical_rate_bonus() {
    let behavior = ParsedBehavior::new(
        60259,
        "SupplyShield2",
        vec![31270012, 102, 1800, 0, 102, 6500, 201, 900],
    );
    let managers = BattleManagers::default();
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 11,
            active_skill_id: 31270124,
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

    assert!(
        matches!(ops.as_slice(), [RuleOp::Command(BattleCommand::Shield(command))]
        if command.multiplier_bonus == Some((AttrId::CriticalRate, 900)))
    );
}

#[test]
fn team_share_behavior_keeps_its_exact_shield_scope() {
    let behavior = ParsedBehavior::new(
        60290,
        "SupplyTeamShareShield",
        vec![31430144, 102, 2800, 0, 102, 12500],
    );
    let managers = BattleManagers::default();
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31430144,
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

    assert!(
        matches!(ops.as_slice(), [RuleOp::Command(BattleCommand::Shield(command))]
            if command.scope == ShieldScope::TeamShared)
    );
    assert_eq!(
        super::super::registry::find(&behavior)
            .unwrap()
            .output_owner,
        super::super::registry::OutputOwner::SetupParent
    );
}
