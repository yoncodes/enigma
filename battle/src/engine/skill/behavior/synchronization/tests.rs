use sonettobuf::{Fight, FightEntityInfo, FightTeam};

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
fn props_configure_the_resource_owner_and_reference_data_skills() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                ex_point_type: Some(2),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(100000, "EzioProps"),
        vec![101, 102, 103, 4, 100],
        Vec::new(),
    );
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();

    let ops = Handler::emit_ops(
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
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::ExPoint(
            ExPointCommand::ConfigureSynchronization(ExPointConfigureSynchronization {
                definition: SynchronizationDefinition {
                    skills: [101, 102, 103],
                    action_count: 4,
                    threshold: 100
                },
                ..
            })
        ))]
    ));
    assert_eq!(Handler::references(&behavior).skills, vec![101, 102, 103]);
}

#[test]
fn malformed_props_fail_closed() {
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(100000, "EzioProps"),
        vec![101, 102, 103, 4],
        Vec::new(),
    );

    assert!(definition(&behavior).is_none());
}

#[test]
fn finisher_requires_the_owned_qte_carrier() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1),
                ex_point_type: Some(2),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let origin = crate::engine::skill::rule::CommandOrigin {
        domain: crate::engine::skill::rule::RuleDomain::Behavior,
        key: crate::engine::skill::rule::DefinitionKey::new(100000, "EzioProps"),
    };
    managers
        .execute_ex_point(ExPointCommand::ConfigureSynchronization(
            ExPointConfigureSynchronization {
                origin,
                target_uid: 10,
                definition: SynchronizationDefinition::new([101, 102, 103], 4, 100).unwrap(),
            },
        ))
        .unwrap();
    let pool = TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(100003, "EzioBigSkillEnd"),
        vec![9000],
        Vec::new(),
    );
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();

    assert!(
        Handler::emit_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: -1,
                active_skill_id: 103,
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
        .is_none()
    );
}

#[test]
fn qte_damage_rates_follow_the_configured_hp_side() {
    let first = ParsedBehavior::from_spec(
        BehaviorSpec::new(100001, "EzioBigSkillTyp1"),
        vec![1400, 500, 500],
        Vec::new(),
    );
    let second = ParsedBehavior::from_spec(
        BehaviorSpec::new(100002, "EzioBigSkillTyp2"),
        vec![1400, 500, 500],
        Vec::new(),
    );
    let finisher = ParsedBehavior::from_spec(
        BehaviorSpec::new(100003, "EzioBigSkillEnd"),
        vec![9000],
        Vec::new(),
    );

    assert_eq!(damage_rate(&first, 49, 100), Some((1900, 1000)));
    assert_eq!(damage_rate(&first, 50, 100), Some((1400, 1000)));
    assert_eq!(damage_rate(&second, 49, 100), Some((1400, 1000)));
    assert_eq!(damage_rate(&second, 50, 100), Some((1900, 1000)));
    assert_eq!(damage_rate(&finisher, 1, 100), Some((9000, 1000)));
}
