use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::{
    event::{kind::EventKind, subscription::SubscriptionKey},
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

#[test]
fn grants_one_layer_per_configured_hp_loss_count() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(30006, "LostLife"),
    };
    let tracker = BuffGrant {
        origin,
        source_uid: 10,
        target_uid: 10,
        buff_id: 434821,
        amount: None,
        occurrences: 1,
        child_uid_reservations: 0,
    };
    let tracker_uid = managers
        .execute_buff(BuffCommand::Grant(tracker))
        .unwrap()
        .change
        .added
        .unwrap()
        .buff
        .uid
        .unwrap();
    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 99,
        buff_uid: tracker_uid,
        buff_id: 434821,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::RoundEndFinalSettlement,
            DefinitionKey::new(1019, "LostHpCountAddBuff"),
        ),
        act_type: "LostHpCountAddBuff".to_owned(),
        effect_time: 307,
        effect_condition: 0,
        args: vec![434811, 2],
        raw: "1019#434811#2".to_owned(),
    };

    let hp_lost = BattleEvent::HpLost {
        origin,
        source_uid: 10,
        skill_id: 30006,
        target_uid: 10,
        amount: 10,
        damage_from: crate::engine::manager::hp::HurtDamageFromType::SkillEffect,
        buff_uid: None,
    };
    for _ in 0..2 {
        let ops = rule_ops(&managers, &subscriber, &hp_lost).unwrap();
        let [RuleOp::Command(BattleCommand::Buff(command))] = ops.as_slice() else {
            panic!("expected one exact counter command");
        };
        managers.execute_buff(command.clone()).unwrap();
    }

    let settled = BattleEvent::BuffsSettled(Vec::new());
    let ops = rule_ops(&managers, &subscriber, &settled).unwrap();
    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                source_uid: 10,
                target_uid: 10,
                buff_id: 434811,
                amount: Some(1),
                ..
            }))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::AccumulateActValue(
                BuffAccumulateActValue { delta: -2, .. }
            )))
        ]
    ));

    for op in ops {
        let RuleOp::Command(BattleCommand::Buff(command)) = op else {
            unreachable!()
        };
        managers.execute_buff(command).unwrap();
    }
    assert!(
        rule_ops(&managers, &subscriber, &settled)
            .unwrap()
            .is_empty()
    );
}
