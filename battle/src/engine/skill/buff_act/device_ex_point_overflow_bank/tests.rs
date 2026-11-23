use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use crate::engine::{
    event::payload::ExPointChangeEvent,
    manager::BattleManagers,
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

use super::*;

fn managers(stored: i32, ex_point: i32) -> BattleManagers {
    crate::test_support::init_config();
    BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3145),
                current_hp: Some(100),
                ex_point: Some(ex_point),
                ex_point_max: Some(100),
                ex_point_type: Some(ExPointKind::DevicePower.as_wire()),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31450173),
                    from_uid: Some(10),
                    act_info: vec![BuffActInfo {
                        act_id: Some(1149),
                        param: vec![stored],
                        str_param: Some(String::new()),
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn subscriber(managers: &BattleManagers, event: EventKind) -> BuffActSubscriber {
    let feature = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| {
            super::super::is_kind(
                feature,
                super::super::registry::BuffActKind::DeviceExPointOverflowBank,
            )
        })
        .expect("device overflow bank feature should be active");
    super::super::subscriber_from_feature(feature, event).unwrap()
}

#[test]
fn device_overflow_accumulates_only_up_to_the_configured_reserve() {
    let managers = managers(48, 90);
    let subscriber = subscriber(&managers, EventKind::ExPointOverflow);
    let event = BattleEvent::ExPointOverflow(ExPointChangeEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(60292, "AddDeviceExPoint"),
        },
        source_uid: 10,
        target_uid: 10,
        kind: ExPointKind::DevicePower,
        before: 90,
        requested_delta: 15,
        applied_delta: 10,
        after: 100,
        overflow: 5,
    });

    let ops = rule_ops(&managers, &subscriber, &event).expect("device overflow state is valid");
    let [op] = ops.as_slice() else {
        panic!("device overflow should emit one state command");
    };
    assert!(matches!(
        &op.op,
        RuleOp::Command(BattleCommand::Buff(BuffCommand::AccumulateCappedActState(
            BuffAccumulateCappedActState {
                act_id: 1149,
                delta: 2,
                maximum: 50,
                ..
            }
        )))
    ));
}

#[test]
fn replenishment_uses_device_energy_and_reduces_only_the_applied_amount() {
    let managers = managers(15, 90);
    let subscriber = subscriber(&managers, EventKind::PostActionRefill);
    let replenishment = replenishment_rule_ops(&managers, &subscriber).unwrap();
    let [op] = replenishment.as_slice() else {
        panic!("stored device energy should schedule one replenishment");
    };
    assert!(matches!(
        &op.op,
        RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(
            ExPointChange { delta: 15, .. }
        )))
    ));
    let event = BattleEvent::ExPointChanged(ExPointChangeEvent {
        origin: super::super::command_origin(&subscriber).unwrap(),
        source_uid: 10,
        target_uid: 10,
        kind: ExPointKind::DevicePower,
        before: 90,
        requested_delta: 15,
        applied_delta: 10,
        after: 100,
        overflow: 5,
    });

    let ops = rule_ops(&managers, &subscriber, &event).unwrap();
    assert!(matches!(
        &ops[0].op,
        RuleOp::Command(BattleCommand::Buff(BuffCommand::SetInternalState(
            BuffSetState { act_info: Some(info), .. }
        ))) if info[0].param == [5]
    ));
    assert!(matches!(
        &ops[1].op,
        RuleOp::BuffActInfoMarker(BuffActInfoMarkerResult {
            act_id: 1149,
            params,
            ..
        }) if params == &[5]
    ));
}
