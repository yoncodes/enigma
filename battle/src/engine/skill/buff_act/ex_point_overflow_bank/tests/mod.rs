use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use crate::engine::{
    event::{kind::EventKind, payload::ExPointChangeEvent, subscription::SubscriptionKey},
    manager::{BattleManagers, ex_point::ExPointKind},
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

use super::*;

mod phase;

fn subscriber(event: EventKind) -> BuffActSubscriber {
    BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 31270400,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            event,
            crate::engine::skill::rule::DefinitionKey::new(806, "ExPointOverflowBank"),
        ),
        act_type: "ExPointOverflowBank".to_owned(),
        effect_time: 18,
        effect_condition: 0,
        args: vec![2],
        raw: "806#2".to_owned(),
    }
}

fn managers_with_bank(stored: i32, ex_point: i32) -> BattleManagers {
    BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3127),
                current_hp: Some(100),
                ex_point: Some(ex_point),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31270400),
                    from_uid: Some(10),
                    act_common_params: Some(format!("806#{stored}")),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    })
}

#[test]
fn overflow_capacity_is_clamped() {
    assert_eq!(stored_overflow(2, 3, 0), 2);
    assert_eq!(stored_overflow(2, 3, 2), 1);
    assert_eq!(stored_overflow(1, 3, 3), 0);
}

#[test]
fn overflow_emits_owned_state_and_value_marker_ops() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31250161),
                    act_common_params: Some("806#1".to_owned()),
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
        buff_id: 31250161,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::ExPointOverflow,
            crate::engine::skill::rule::DefinitionKey::new(806, "ExPointOverflowBank"),
        ),
        act_type: "ExPointOverflowBank".to_owned(),
        effect_time: 18,
        effect_condition: 0,
        args: vec![3],
        raw: "806#3".to_owned(),
    };
    let event = BattleEvent::ExPointOverflow(ExPointChangeEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(1, "AddExPoint"),
        },
        source_uid: 10,
        target_uid: 10,
        kind: ExPointKind::Common,
        before: 5,
        requested_delta: 2,
        applied_delta: 0,
        after: 5,
        overflow: 2,
    });

    let ops = rule_ops(&managers, &subscriber, &event).unwrap();

    assert!(ops.iter().all(|op| {
        op.scope == crate::engine::skill::buff_act::BuffActFrameScope::SubscriberFrame
            && op.frame_owner == crate::engine::skill::buff_act::BuffActFrameOwner::Event
            && op.group_with_siblings
    }));
    assert!(matches!(
        &ops[0].op,
        RuleOp::Command(BattleCommand::Buff(BuffCommand::SetStateSnapshot(
            BuffSetState {
                params: Some(params),
                ..
            }
        ))) if params == "806#3"
    ));
    assert_eq!(ops.len(), 1);
}

#[test]
fn replenishment_requests_the_stored_amount_through_the_resource_manager() {
    let managers = managers_with_bank(2, 3);

    let subscriber = subscriber(EventKind::PostActionRefill);
    let ops = replenishment_rule_ops(&managers, &subscriber).unwrap();

    assert_eq!(ops.len(), 1);
    assert_eq!(subscriber.owner_uid, 10);
    assert_eq!(subscriber.key.event, EventKind::PostActionRefill);
    assert!(matches!(
        ops[0].op,
        RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(
            ExPointChange {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey {
                        opcode: 806,
                        type_name: "ExPointOverflowBank"
                    }
                },
                source_uid: 10,
                target_uid: 10,
                delta: 2,
                ..
            }
        )))
    ));
}

#[test]
fn replenishment_consumes_only_the_amount_the_manager_applied() {
    let managers = managers_with_bank(2, 4);
    let event = BattleEvent::ExPointChanged(ExPointChangeEvent {
        origin: super::super::command_origin(&subscriber(EventKind::ExPointChanged)).unwrap(),
        source_uid: 10,
        target_uid: 10,
        kind: ExPointKind::Common,
        before: 4,
        requested_delta: 2,
        applied_delta: 1,
        after: 5,
        overflow: 1,
    });

    let ops = rule_ops(&managers, &subscriber(EventKind::ExPointChanged), &event).unwrap();

    assert!(
        ops.iter().all(|op| {
            op.scope == crate::engine::skill::buff_act::BuffActFrameScope::CausingFrame
        })
    );
    assert!(matches!(
        &ops[0].op,
        RuleOp::Command(BattleCommand::Buff(BuffCommand::SetStateSnapshot(
            BuffSetState {
                params: Some(params),
                ..
            }
        ))) if params == "806#1"
    ));
    assert_eq!(ops.len(), 1);
}

#[test]
fn blocked_or_self_overflowed_replenishment_keeps_bank_state() {
    let managers = managers_with_bank(2, 5);
    assert!(
        replenishment_rule_ops(&managers, &subscriber(EventKind::PostActionRefill))
            .unwrap()
            .is_empty()
    );
    let changed_subscriber = subscriber(EventKind::ExPointChanged);
    let origin = super::super::command_origin(&changed_subscriber).unwrap();
    let blocked = BattleEvent::ExPointChanged(ExPointChangeEvent {
        origin,
        source_uid: 10,
        target_uid: 10,
        kind: ExPointKind::Common,
        before: 5,
        requested_delta: 2,
        applied_delta: 0,
        after: 5,
        overflow: 0,
    });
    assert!(
        rule_ops(&managers, &changed_subscriber, &blocked)
            .unwrap()
            .is_empty()
    );

    let own_overflow = BattleEvent::ExPointOverflow(ExPointChangeEvent {
        origin,
        source_uid: 10,
        target_uid: 10,
        kind: ExPointKind::Common,
        before: 5,
        requested_delta: 2,
        applied_delta: 0,
        after: 5,
        overflow: 2,
    });
    assert!(
        rule_ops(
            &managers,
            &subscriber(EventKind::ExPointOverflow),
            &own_overflow
        )
        .unwrap()
        .is_empty()
    );
}
