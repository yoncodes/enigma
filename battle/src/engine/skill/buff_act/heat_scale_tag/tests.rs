use sonettobuf::{BuffActInfo, BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::{
    event::kind::EventKind,
    manager::gauge::{GaugeBatchId, GaugeCommand, GaugeOperation},
    skill::buff_act,
};

#[test]
fn gauge_change_persists_the_visible_accumulation_counter() {
    crate::test_support::init_config();
    let mut managers = BattleManagers::seeded(&Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![
                    BuffInfo {
                        buff_id: Some(31340003),
                        uid: Some(20),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(31340005),
                        uid: Some(21),
                        from_uid: Some(10),
                        act_info: vec![BuffActInfo {
                            act_id: Some(1050),
                            param: vec![0],
                            str_param: Some(String::new()),
                        }],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let tag = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| buff_act::is_kind(feature, BuffActKind::HeatScaleTag))
        .unwrap();
    let origin = buff_act::feature_command_origin(&tag).unwrap();
    let subscriber = buff_act::subscriber_from_feature(tag, EventKind::GaugeChanged).unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            lingering_glow::key(1),
            GaugeOperation::Enable { max: Some(150) },
        ))
        .unwrap();
    let change = managers
        .execute_gauge(GaugeCommand::new(
            origin,
            lingering_glow::key(1),
            GaugeOperation::AccumulateProgress { raw_amount: 40_000 },
        ))
        .unwrap();
    let event = change.events().into_iter().next().unwrap();

    let ops = rule_ops(&managers, &subscriber, &event).unwrap();

    assert!(matches!(
        &ops[0].op,
        RuleOp::Command(BattleCommand::Buff(BuffCommand::SetInternalState(
            BuffSetState { act_info: Some(info), .. }
        ))) if info[0].param == [40]
    ));
    assert!(matches!(
        &ops[1].op,
        RuleOp::BuffActInfoMarker(marker) if marker.act_id == 1050 && marker.params == [40]
    ));
    let RuleOp::Command(BattleCommand::Buff(command)) = &ops[0].op else {
        unreachable!();
    };
    managers.execute_buff(command.clone()).unwrap();
    assert_eq!(
        managers.buff.snapshot(10, 21).unwrap().act_info[0].param,
        [40]
    );
}

#[test]
fn contribution_events_advance_the_visible_counter_in_order() {
    crate::test_support::init_config();
    let mut managers = BattleManagers::seeded(&Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![
                    BuffInfo {
                        buff_id: Some(31340003),
                        uid: Some(20),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                    BuffInfo {
                        buff_id: Some(31340005),
                        uid: Some(21),
                        from_uid: Some(10),
                        act_info: vec![BuffActInfo {
                            act_id: Some(1050),
                            param: vec![86],
                            str_param: Some(String::new()),
                        }],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let tag = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| buff_act::is_kind(feature, BuffActKind::HeatScaleTag))
        .unwrap();
    let origin = buff_act::feature_command_origin(&tag).unwrap();
    let subscriber = buff_act::subscriber_from_feature(tag, EventKind::GaugeChanged).unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            lingering_glow::key(1),
            GaugeOperation::Enable { max: Some(150) },
        ))
        .unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            lingering_glow::key(1),
            GaugeOperation::AccumulateRawValue {
                amount: 86_000,
                stream: 1062,
            },
        ))
        .unwrap();
    let batch = GaugeBatchId::new(origin, lingering_glow::key(1), 0, 21);
    let first = managers
        .start_raw_gauge_batch(
            batch,
            vec![
                GaugeCommand::new(
                    origin,
                    lingering_glow::key(1),
                    GaugeOperation::AccumulateRawValue {
                        amount: 10_000,
                        stream: 1062,
                    },
                ),
                GaugeCommand::new(
                    origin,
                    lingering_glow::key(1),
                    GaugeOperation::AccumulateRawValue {
                        amount: 7_000,
                        stream: 1062,
                    },
                ),
            ],
        )
        .unwrap();

    let mut counters = Vec::new();
    let mut change = first;
    for index in 0..2 {
        let event = change.events().into_iter().next().unwrap();
        let ops = rule_ops(&managers, &subscriber, &event).unwrap();
        let RuleOp::Command(BattleCommand::Buff(command)) = &ops[0].op else {
            unreachable!();
        };
        let RuleOp::BuffActInfoMarker(marker) = &ops[1].op else {
            unreachable!();
        };
        counters.push(marker.params[0]);
        managers.execute_buff(command.clone()).unwrap();
        if index == 0 {
            change = managers.continue_raw_gauge_batch(batch).unwrap();
        }
    }
    managers.commit_raw_gauge_batch(batch).unwrap();

    assert_eq!(counters, [96, 103]);
}

#[test]
fn defeated_main_target_uses_the_current_alive_enemy_count() {
    crate::test_support::init_config();
    let managers = BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                position: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    team_type: Some(2),
                    position: Some(1),
                    current_hp: Some(0),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    team_type: Some(2),
                    position: Some(2),
                    current_hp: Some(0),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-3),
                    team_type: Some(2),
                    position: Some(3),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    });

    assert_eq!(enemy_count_for_gain(&managers, 2, -1), Some(1));
}
