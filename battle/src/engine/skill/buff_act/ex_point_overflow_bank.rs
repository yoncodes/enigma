use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffSetState},
        ex_point::{ExPointChange, ExPointCommand},
    },
    skill::{
        buff_act::BuffActRuleOp,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn replenishment_rule_ops(managers: &BattleManagers) -> Vec<(BuffActSubscriber, RuleOp)> {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| {
            super::is_kind(feature, super::registry::BuffActKind::ExPointOverflowBank)
        })
        .filter_map(|feature| {
            let subscriber = super::subscriber_from_feature(feature, EventKind::ExPointChanged)?;
            let stored = current(
                &managers
                    .buff
                    .snapshot(subscriber.owner_uid, subscriber.buff_uid)?,
            );
            if stored <= 0
                || !subscriber.owner_alive
                || managers.ex_point.is_full(subscriber.owner_uid)
                || managers.buff.has_buff_act_kind(
                    subscriber.owner_uid,
                    super::registry::BuffActKind::ExPointCantAdd,
                )
                || managers.buff.has_buff_act_kind(
                    subscriber.owner_uid,
                    super::registry::BuffActKind::TransferAddExPoint,
                )
            {
                return None;
            }
            let origin = super::command_origin(&subscriber)?;
            let owner_uid = subscriber.owner_uid;
            Some((
                subscriber,
                RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(
                    ExPointChange {
                        origin,
                        source_uid: owner_uid,
                        target_uid: owner_uid,
                        delta: stored,
                        config_effect: 0,
                        effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
                    },
                ))),
            ))
        })
        .collect()
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<BuffActRuleOp>> {
    if !super::subscriber_is_kind(
        subscriber,
        super::registry::BuffActKind::ExPointOverflowBank,
    ) {
        return Some(Vec::new());
    }
    match event {
        BattleEvent::ExPointOverflow(change) => store_rule_ops(managers, subscriber, change),
        BattleEvent::ExPointChanged(change) => consume_rule_ops(managers, subscriber, change),
        _ => Some(Vec::new()),
    }
}

fn store_rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    change: &crate::engine::event::payload::ExPointChangeEvent,
) -> Option<Vec<BuffActRuleOp>> {
    if change.overflow <= 0
        || change.origin == super::command_origin(subscriber)?
        || !change
            .kind
            .can_bank_overflow(change.kind, change.target_uid, subscriber.owner_uid)
    {
        return Some(Vec::new());
    }
    let capacity = subscriber.args.first().copied()? * subscriber.amount.max(1);
    let buff = managers
        .buff
        .snapshot(subscriber.owner_uid, subscriber.buff_uid)?;
    let current = current(&buff);
    let stored = stored_overflow(change.overflow, capacity, current);
    if stored <= 0 {
        return Some(Vec::new());
    }
    Some(vec![BuffActRuleOp::grouped_event(RuleOp::Command(
        BattleCommand::Buff(BuffCommand::SetStateSnapshot(BuffSetState {
            ex_info: None,
            origin: super::command_origin(subscriber)?,
            target_uid: subscriber.owner_uid,
            buff_uid: subscriber.buff_uid,
            params: Some(format!(
                "{}#{}",
                subscriber.key.definition.opcode,
                current + stored
            )),
            act_info: None,
        })),
    ))])
}

fn consume_rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    change: &crate::engine::event::payload::ExPointChangeEvent,
) -> Option<Vec<BuffActRuleOp>> {
    if change.origin != super::command_origin(subscriber)?
        || change.source_uid != subscriber.owner_uid
        || change.target_uid != subscriber.owner_uid
        || change.applied_delta <= 0
    {
        return Some(Vec::new());
    }
    let buff = managers
        .buff
        .snapshot(subscriber.owner_uid, subscriber.buff_uid)?;
    let current = current(&buff);
    let restored = change.applied_delta.min(current);
    if restored <= 0 {
        return Some(Vec::new());
    }
    Some(vec![BuffActRuleOp::causing(RuleOp::Command(
        BattleCommand::Buff(BuffCommand::SetStateSnapshot(BuffSetState {
            ex_info: None,
            origin: super::command_origin(subscriber)?,
            target_uid: subscriber.owner_uid,
            buff_uid: subscriber.buff_uid,
            params: Some(format!(
                "{}#{}",
                subscriber.key.definition.opcode,
                current - restored
            )),
            act_info: None,
        })),
    ))])
}

fn current(buff: &sonettobuf::BuffInfo) -> i32 {
    buff.act_common_params
        .as_deref()
        .and_then(|raw| raw.rsplit('#').next())
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or_default()
}

fn stored_overflow(overflow: i32, capacity: i32, current: i32) -> i32 {
    overflow.min((capacity - current).max(0))
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    use crate::engine::{
        event::{kind::EventKind, payload::ExPointChangeEvent, subscription::SubscriptionKey},
        manager::{BattleManagers, ex_point::ExPointKind},
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };

    use super::*;

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

        let ops = replenishment_rule_ops(&managers);

        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].0.owner_uid, 10);
        assert_eq!(ops[0].0.key.event, EventKind::ExPointChanged);
        assert!(matches!(
            ops[0].1,
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

        assert!(ops.iter().all(|op| {
            op.scope == crate::engine::skill::buff_act::BuffActFrameScope::CausingFrame
        }));
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
        assert!(replenishment_rule_ops(&managers).is_empty());
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
}
