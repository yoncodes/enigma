use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::{event::subscription::SubscriptionKey, skill::rule::DefinitionKey};

#[test]
fn below_threshold_does_not_add_the_configured_card() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3124),
                current_hp: Some(100),
                ex_point: Some(1),
                ex_point_type: Some(3),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31242140),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 31242140,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::RoundStartCard,
            DefinitionKey::new(10001, "AdrenalineAddCard"),
        ),
        act_type: "AdrenalineAddCard".to_owned(),
        effect_time: 105,
        effect_condition: 0,
        args: vec![2, 6, 10, 312451011, 312451023, 312451031],
        raw: "10001#2,6,10#312451011,312451023,312451031".to_owned(),
    };

    let ops = rule_ops(
        &BattleManagers::seeded(&fight),
        &subscriber,
        &BattleEvent::Kind(EventKind::RoundStartCard),
    )
    .unwrap();

    assert!(ops.is_empty());
    assert!(supports(&[10, 31242103]));
    assert!(supports(&[2, 6, 10, 312451011, 312451023, 312451031]));
    assert!(!supports(&[10]));
    assert!(!supports(&[6, 2, 312451023, 312451011]));
}

#[test]
fn grouped_thresholds_cross_from_zero_through_the_terminal_card() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3124),
                current_hp: Some(100),
                ex_point: Some(10),
                ex_point_type: Some(3),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31242140),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 31242140,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::RoundStartCard,
            DefinitionKey::new(10001, "AdrenalineAddCard"),
        ),
        act_type: "AdrenalineAddCard".to_owned(),
        effect_time: 105,
        effect_condition: 0,
        args: vec![2, 6, 10, 312451011, 312451023, 312451031],
        raw: "10001#2,6,10#312451011,312451023,312451031".to_owned(),
    };
    let event = BattleEvent::Kind(EventKind::RoundStartCard);
    let mut managers = BattleManagers::seeded(&fight);
    let crossed = rule_ops(&managers, &subscriber, &event).unwrap();
    assert!(matches!(
        crossed.as_slice(),
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::AccumulateActValue(
                BuffAccumulateActValue { delta: 1, .. }
            ))),
            RuleOp::Command(BattleCommand::Card(CardCommand::AddTemporary(
                CardAddTemporary {
                    skill_id: 312451011,
                    hero_id: Some(3124),
                    kind: TemporaryCardKind::ConfiguredSkill3,
                    ..
                }
            ))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::AccumulateActValue(
                BuffAccumulateActValue { delta: 1, .. }
            ))),
            RuleOp::Command(BattleCommand::Card(CardCommand::AddTemporary(
                CardAddTemporary {
                    skill_id: 312451023,
                    hero_id: Some(3124),
                    kind: TemporaryCardKind::ConfiguredSkill3,
                    ..
                }
            ))),
            RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Set(ExPointSet {
                target_uid: 10,
                value: 0,
                ..
            }))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::AccumulateActValue(
                BuffAccumulateActValue { delta: -2, .. }
            ))),
            RuleOp::Command(BattleCommand::Card(CardCommand::AddGenerated(
                CardAddGenerated {
                    target_uid: 10,
                    skill_id: 312451031,
                    hero_id: Some(3124),
                    team_type: 1,
                    ..
                }
            )))
        ]
    ));
    let mut generated = None;
    for op in crossed {
        match op {
            RuleOp::Command(BattleCommand::ExPoint(command)) => {
                managers.execute_ex_point(command).unwrap();
            }
            RuleOp::Command(BattleCommand::Buff(command)) => {
                managers.execute_buff(command).unwrap();
            }
            RuleOp::Command(BattleCommand::Card(command)) => {
                generated = managers.execute_card(command).unwrap().added;
            }
            _ => unreachable!(),
        }
    }
    assert_eq!(managers.ex_point.get(10), 0);
    assert_eq!(managers.buff.act_value(20, 10001), 0);
    let card = generated.unwrap();
    assert_eq!(card.hero_id, Some(3124));
    assert_eq!(card.temp_card, Some(false));
}
