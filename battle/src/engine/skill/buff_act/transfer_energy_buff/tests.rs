use sonettobuf::{Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::{
    event::{kind::EventKind, payload::ExPointChangeEvent, subscription::SubscriptionKey},
    skill::rule::{DefinitionKey, RuleDomain},
};

#[test]
fn parses_the_config_owned_transfer_rule() {
    assert_eq!(referenced_buff(&[31280113, 1, 20, 3]), Some(31280113));
    assert_eq!(referenced_buff(&[31280113, 1, 20]), None);
    assert_eq!(referenced_buff(&[0, 1, 20, 3]), None);
}

#[test]
fn same_team_moxie_overflow_grants_the_configured_buff_amount() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    team_type: Some(1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(100),
                    team_type: Some(1),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                team_type: Some(2),
                ..Default::default()
            }],
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
        buff_id: 31280118,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::ExPointOverflow,
            DefinitionKey::new(1033, "TransferEnergyBuff"),
        ),
        act_type: "TransferEnergyBuff".into(),
        effect_time: 0,
        effect_condition: 0,
        args: vec![31280113, 1, 20, 3],
        raw: "1033#31280113#1#20#3".into(),
    };
    let event = |target_uid| {
        BattleEvent::ExPointOverflow(ExPointChangeEvent {
            spent: false,
            origin: crate::engine::skill::rule::CommandOrigin {
                domain: RuleDomain::Lifecycle,
                key: DefinitionKey::new(0, "RoundRefill"),
            },
            source_uid: target_uid,
            target_uid,
            kind: ExPointKind::Common,
            before: 5,
            requested_delta: 2,
            applied_delta: 0,
            after: 5,
            overflow: 2,
        })
    };

    let ops = rule_ops(&managers, &pool, &subscriber, &event(11)).unwrap();
    assert!(matches!(
        ops.as_slice(),
        [BuffActRuleOp {
            op: RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                buff_id: 31280113,
                amount: Some(40),
                ..
            }))),
            ..
        }]
    ));
    assert!(
        rule_ops(&managers, &pool, &subscriber, &event(-1))
            .unwrap()
            .is_empty()
    );
}
