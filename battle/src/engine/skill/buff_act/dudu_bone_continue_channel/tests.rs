use sonettobuf::{Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::{
    event::bus::EventBus,
    runtime::executor::execute_rule_op,
    skill::{buff_act, subscriber},
};

#[test]
fn channel_keeps_elapsed_rounds_in_buff_ex_info_and_ends_when_moxie_is_short() {
    crate::test_support::init_config();
    let entity = |uid, team_type, ex_point| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(1_000),
        ex_point: Some(ex_point),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 5)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, 0)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    managers.buff.add(&managers.hp, 10, 10, 31040014, 0);
    let subscriber = subscriber::for_active_buffs(&managers, EventKind::SmallRoundEnd)
        .into_iter()
        .find(|subscriber| {
            buff_act::subscriber_is_kind(subscriber, BuffActKind::DuduBoneContinueChannel)
        })
        .unwrap();
    let event = BattleEvent::Kind(EventKind::SmallRoundEnd);
    let mut bus = EventBus::default();

    let first = rule_ops(&managers, &pool, &subscriber, &event).unwrap();
    assert_eq!(first.len(), 2);
    for op in first {
        execute_rule_op(&mut managers, &mut bus, op).unwrap();
    }
    assert_eq!(managers.ex_point.get(10), 5);
    assert_eq!(
        managers
            .buff
            .snapshot(10, subscriber.buff_uid)
            .unwrap()
            .ex_info,
        Some(1)
    );
    assert_eq!(managers.buff.buff_id_amount(-1, 31040013), 1);

    let second = rule_ops(&managers, &pool, &subscriber, &event).unwrap();
    assert_eq!(second.len(), 3);
    for op in second {
        execute_rule_op(&mut managers, &mut bus, op).unwrap();
    }
    assert_eq!(managers.ex_point.get(10), 1);
    assert_eq!(
        managers
            .buff
            .snapshot(10, subscriber.buff_uid)
            .unwrap()
            .ex_info,
        Some(2)
    );
    assert_eq!(managers.buff.buff_id_amount(-1, 31040013), 3);
    assert_eq!(compound_offset(&managers, 10), 2);

    let ending = rule_ops(&managers, &pool, &subscriber, &event).unwrap();
    assert!(matches!(
        ending.as_slice(),
        [
            RuleOp::Command(BattleCommand::ExPoint(_)),
            RuleOp::Skill(SkillInvocation {
                plan: SkillRequest {
                    skill_id: 310401311,
                    ..
                },
                ..
            }),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(_)))
        ]
    ));
}
