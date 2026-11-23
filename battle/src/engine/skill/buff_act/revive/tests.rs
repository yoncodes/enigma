use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::{
    event::{kind::EventKind, payload::EntityDiedEvent, subscription::SubscriptionKey},
    skill::rule::DefinitionKey,
};

#[test]
fn source_scaled_revive_emits_one_atomic_transaction() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(0),
                attr: Some(HeroAttribute {
                    hp: Some(20_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31250181),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 31250181,
        team_type: 1,
        owner_alive: false,
        amount: 1,
        key: SubscriptionKey::new(EventKind::EntityDied, DefinitionKey::new(1043, "Revive")),
        act_type: "Revive".to_owned(),
        effect_time: 12,
        effect_condition: 0,
        args: vec![0, AttrId::Hp as i32, 300],
        raw: "1043#0#101#300".to_owned(),
    };

    assert!(matches!(
        rule_ops(
            &managers,
            &subscriber,
            &BattleEvent::EntityDied(EntityDiedEvent {
                source_uid: -1,
                target_uid: 10,
            }),
        )
        .unwrap()
        .as_slice(),
        [RuleOp::Command(BattleCommand::Revive(ReviveCommand {
            amount: 6_000,
            ..
        }))]
    ));
}

#[test]
fn lethal_cure_recovers_configured_max_hp_and_consumes_its_buff() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(0),
                attr: Some(HeroAttribute {
                    hp: Some(20_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(433021),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 433021,
        team_type: 1,
        owner_alive: false,
        amount: 1,
        key: SubscriptionKey::new(EventKind::EntityDied, DefinitionKey::new(512, "Cure")),
        act_type: "Cure".to_owned(),
        effect_time: 12,
        effect_condition: 0,
        args: vec![1, AttrId::Hp as i32, 330],
        raw: "512#1#101#330".to_owned(),
    };

    assert!(matches!(
        rule_ops(
            &managers,
            &subscriber,
            &BattleEvent::EntityDied(EntityDiedEvent {
                source_uid: -1,
                target_uid: 10,
            }),
        )
        .unwrap()
        .as_slice(),
        [RuleOp::Command(BattleCommand::Revive(ReviveCommand {
            amount: 6_600,
            buff_uid: 20,
            ..
        }))]
    ));
}
