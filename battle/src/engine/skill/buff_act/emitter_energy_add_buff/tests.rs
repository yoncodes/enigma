use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::{
    event::kind::EventKind,
    manager::{BattleManagers, emitter, gauge::GaugeCommand},
    skill::{buff_act, rule::output::BattleCommand},
};

#[test]
fn inspiration_thresholds_activate_visible_attack_buffs() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(emitter::UID),
                team_type: Some(1),
                current_hp: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(30),
                    buff_id: Some(31080151),
                    from_uid: Some(emitter::UID),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let subscriber = crate::engine::skill::subscriber::for_active_buffs(
        &managers,
        EventKind::PlayerActionsResolved,
    )
    .into_iter()
    .find(|subscriber| subscriber.args.first() == Some(&3))
    .unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            buff_act::command_origin(&subscriber).unwrap(),
            impromptu::inspiration_key(emitter::UID),
            crate::engine::manager::gauge::GaugeOperation::Enable { max: None },
        ))
        .unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            buff_act::command_origin(&subscriber).unwrap(),
            impromptu::inspiration_key(emitter::UID),
            crate::engine::manager::gauge::GaugeOperation::ChangeValue { delta: 20 },
        ))
        .unwrap();

    let event = BattleEvent::PlayerActionsResolved {
        team: 1,
        emitter_uid: emitter::UID,
    };
    let ops = rule_ops(&managers, &subscriber, &event).unwrap();
    assert_eq!(ops.len(), 4);
    for op in ops {
        let RuleOp::Command(BattleCommand::Buff(command)) = op.op else {
            panic!("expected a buff command")
        };
        let changes = managers.execute_buff(command).unwrap();
        assert!(changes.is_wire_visible());
        assert!(!changes.events().is_empty());
    }
    for buff_id in 31080152..=31080155 {
        assert!(managers.buff.has_buff_id(emitter::UID, buff_id));
    }
}
