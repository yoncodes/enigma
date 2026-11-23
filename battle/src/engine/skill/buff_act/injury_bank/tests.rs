use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::{
    event::{bus::EventBus, kind::EventKind, subscription::SubscriptionKey},
    runtime::executor::execute_rule_op,
};

#[test]
fn injury_bank_cap_is_a_buff_command_not_private_scanner_state() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(30800141),
                    from_uid: Some(10),
                    act_common_params: Some("770#0".to_owned()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 30800141,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::BuffAdded,
            crate::engine::skill::rule::DefinitionKey::new(770, "InjuryBank"),
        ),
        act_type: "InjuryBank".to_owned(),
        effect_time: 0,
        effect_condition: 0,
        args: vec![101, 200, 30800161, 30, 100, 100],
        raw: "770#101#200#30800161#30#100#100".to_owned(),
    };
    let output = rule_op(&managers, &subscriber).unwrap();

    execute_rule_op(&mut managers, &mut EventBus::default(), output).unwrap();

    assert_eq!(
        managers
            .buff
            .snapshot(10, 20)
            .unwrap()
            .act_common_params
            .as_deref(),
        Some("770#0#200")
    );
}

#[test]
fn stored_injury_crossing_a_threshold_heals_each_living_ally() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(800),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(30800141),
                        from_uid: Some(10),
                        act_common_params: Some("770#20#200".to_owned()),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    team_type: Some(1),
                    current_hp: Some(500),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 30800141,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::HpLost,
            crate::engine::skill::rule::DefinitionKey::new(770, "InjuryBank"),
        ),
        act_type: "InjuryBank".to_owned(),
        effect_time: 0,
        effect_condition: 0,
        args: vec![101, 200, 30800161, 30, 100, 100],
        raw: "770#101#200#30800161#30#100#100".to_owned(),
    };
    let event = BattleEvent::HpLost {
        origin: crate::engine::skill::rule::CommandOrigin {
            domain: crate::engine::skill::rule::RuleDomain::Behavior,
            key: crate::engine::skill::rule::DefinitionKey::new(1, "Damage"),
        },
        source_uid: -1,
        skill_id: 1,
        target_uid: 10,
        amount: 100,
        buff_uid: None,
    };

    let ops = runtime_rule_ops(&managers, &pool, &subscriber, &event).unwrap();

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::SetState(BuffSetState {
                params: Some(params),
                ..
            }))),
            RuleOp::BuffFeatureMarker { effect_num: 10, .. },
            RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(HpHeal {
                target_uid: 10,
                amount: 100,
                ..
            }))),
            RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(HpHeal {
                target_uid: 11,
                amount: 100,
                ..
            })))
        ] if params == "770#30#200"
    ));
}

#[test]
fn origin_attack_reads_the_grant_snapshot_as_a_flat_value() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(30800111),
                    from_uid: Some(11),
                    act_common_params: Some("767#317".to_owned()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let feature = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| buff_act::is_kind(feature, BuffActKind::AttrFixFromInjuryBank))
        .unwrap();

    assert_eq!(
        snapshotted_attribute_delta(&feature, AttrId::Attack, &managers.buff),
        317
    );
    assert_eq!(managers.origin_attribute(10, AttrId::Attack), 317);
}
