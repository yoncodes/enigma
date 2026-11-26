use super::*;
use crate::engine::{
    manager::{
        buff::BuffGrant,
        gauge::{GaugeCommand, GaugeOperation},
    },
    runtime::{change::BattleChange, executor::execute_rule_op},
    skill::rule::{DefinitionKey, RuleDomain},
};
use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

#[test]
fn setup_reports_only_applied_threshold_rewards() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(20),
                current_hp: Some(100),
                ex_point: Some(5),
                ex_point_max: Some(5),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_buff(crate::engine::manager::buff::BuffCommand::Grant(
            BuffGrant {
                origin: origin(),
                source_uid: 20,
                target_uid: 20,
                buff_id: 308802111,
                amount: Some(1),
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))
        .unwrap();
    let key = crate::engine::mechanic::bloodtithe::rule::key(1);

    managers
        .execute_gauge(GaugeCommand::new(
            origin(),
            key,
            GaugeOperation::Enable { max: Some(56) },
        ))
        .unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            origin(),
            key,
            GaugeOperation::ChangeValue { delta: 8 },
        ))
        .unwrap();
    let active = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| feature.buff_id == 308802111)
        .unwrap();
    let settle = |managers: &mut BattleManagers| {
        let mut events = crate::engine::event::bus::EventBus::default();
        setup_rule_ops(managers, &active)
            .unwrap()
            .into_iter()
            .flat_map(|op| {
                execute_rule_op(managers, &mut events, op)
                    .unwrap()
                    .changes()
            })
            .collect::<Vec<_>>()
    };

    assert_eq!(managers.ex_point.get(20), 5);
    assert!(
        !settle(&mut managers)
            .iter()
            .any(|change| matches!(change, BattleChange::BuffFeatureMarker(_)))
    );
    assert_eq!(managers.ex_point.get(20), 5);
    managers.ex_point.set(20, 20, 4, 0);
    assert!(settle(&mut managers).is_empty());
    managers
        .execute_gauge(GaugeCommand::new(
            origin(),
            key,
            GaugeOperation::ChangeValue { delta: 16 },
        ))
        .unwrap();

    assert!(matches!(
        settle(&mut managers).as_slice(),
        [
            BattleChange::BuffFeatureMarker(marker),
            BattleChange::ExPoint(ExPointChanges::Value { change, .. }),
        ] if marker.target_uid == 20
            && marker.effect_num == 308802111
            && marker.buff_act_id == 1021
            && change.applied_delta == 1
    ));
    assert_eq!(managers.ex_point.get(20), 5);
}

#[test]
fn event_does_not_emit_a_marker_before_the_threshold_crosses() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(20),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_buff(crate::engine::manager::buff::BuffCommand::Grant(
            BuffGrant {
                origin: origin(),
                source_uid: 20,
                target_uid: 20,
                buff_id: 308802111,
                amount: Some(1),
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))
        .unwrap();
    let key = crate::engine::mechanic::bloodtithe::rule::key(1);
    managers
        .execute_gauge(GaugeCommand::new(
            origin(),
            key,
            GaugeOperation::Enable { max: Some(56) },
        ))
        .unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            origin(),
            key,
            GaugeOperation::ChangeValue { delta: 7 },
        ))
        .unwrap();
    let event = BattleEvent::SkillAction(crate::engine::skill::action::SkillActionEvent {
        source_uid: 20,
        card_index: 0,
        skill_id: 1,
        target_uid: -1,
        target_uids: vec![-1],
        attacked_target_uids: vec![-1],
        phase: SkillPhase::AfterHit,
        skill_slot: 1,
        is_attack: true,
        rank: 1,
        skill_type: 0,
        effect_tag: 0,
        assassinate: false,
        ignore_riposte: false,
        damage_amount: 0,
        kill_count: 0,
        crit_count: 0,
        guard_break_count: 0,
        additional_moxie: 0,
        extra_skill_kind: 0,
        mode: crate::engine::skill::action::SkillExecutionMode::Active,
        teammate_injury_count: 0,
        teammate_injury_count_not_reset: 0,
        team_injury_count_round: 0,
        card_enchants: Vec::new(),
        buff_additions: Vec::new(),
    });

    assert!(event_rule_ops(&managers, &event).is_empty());
}

#[test]
fn execution_uses_cumulative_positive_gain_after_the_gauge_is_consumed() {
    let mut managers = BattleManagers::default();
    let key = crate::engine::mechanic::bloodtithe::rule::key(1);
    managers
        .execute_gauge(GaugeCommand::new(
            origin(),
            key,
            GaugeOperation::Enable { max: Some(56) },
        ))
        .unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            origin(),
            key,
            GaugeOperation::ChangeValue { delta: 8 },
        ))
        .unwrap();
    managers.ex_point.set(20, 20, 0, 0);
    let command = BloodPoolCountAddExPointCommand {
        origin: origin(),
        key,
        listener_uid: 1,
        listener_opcode: 1021,
        buff_id: 308802111,
        source_uid: 20,
        target_uid: 20,
        threshold: 8,
        amount: 1,
    };

    assert!(execute(&mut managers, command).unwrap().is_some());
    assert_eq!(managers.ex_point.get(20), 1);
    assert!(execute(&mut managers, command).unwrap().is_none());
}

fn origin() -> CommandOrigin {
    CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: DefinitionKey::new(1021, "BloodPoolCountAddExPoint"),
    }
}
