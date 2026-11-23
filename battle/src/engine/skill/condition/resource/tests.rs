use super::*;
use sonettobuf::{Fight, FightEntityInfo, FightTeam};

fn condition(kind: ParsedConditionKind) -> ParsedCondition {
    ParsedCondition {
        opcode: 1,
        type_name: String::new(),
        kind,
        raw_args: Vec::new(),
    }
}

fn attacker_pool() -> TargetPool {
    TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: [10, 11]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    current_hp: Some(1),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn context<'a>(
    event: ResourceEvent,
    source_uid: i64,
    targets: &'a [i64],
    managers: &'a BattleManagers,
    pool: &'a TargetPool,
) -> ResourceConditionContext<'a> {
    ResourceConditionContext {
        event,
        source_uid,
        condition_targets: targets,
        condition_target_code: 103,
        managers,
        pool,
        random_roll: None,
    }
}

#[test]
fn consumed_and_overflow_power_keep_distinct_event_amounts() {
    let managers = BattleManagers::default();
    let pool = attacker_pool();
    let consumed = condition(ParsedConditionKind::PowerConsumed {
        power_id: 1,
        max_count: 99,
    });
    let overflow = condition(ParsedConditionKind::PowerOverflow {
        power_id: 1,
        max_count: 99,
    });
    let event = ResourceEvent::Eureka {
        target_uid: 10,
        eureka_id: 1,
        applied_delta: -2,
        overflow: 3,
    };

    assert_eq!(
        event_conditions_count(&[consumed], context(event, 10, &[10], &managers, &pool)),
        2
    );
    assert_eq!(
        event_conditions_count(&[overflow], context(event, 10, &[10], &managers, &pool)),
        3
    );
}

#[test]
fn other_ally_ex_point_increase_requires_each_complete_positive_threshold() {
    let managers = BattleManagers::default();
    let pool = attacker_pool();
    let condition = condition(ParsedConditionKind::ExPointIncrChange {
        threshold: 5,
        kind: 2,
        scope: ExPointIncreaseScope::OtherAlly,
    });
    let count = |target_uid, delta| {
        event_conditions_count(
            std::slice::from_ref(&condition),
            context(
                ResourceEvent::ExPoint {
                    target_uid,
                    kind: 2,
                    delta,
                },
                10,
                &[10, 11],
                &managers,
                &pool,
            ),
        )
    };

    assert_eq!(count(11, -5), 0);
    assert_eq!(count(11, 4), 0);
    assert_eq!(count(11, 5), 1);
    assert_eq!(count(11, 11), 2);
    assert_eq!(count(10, 5), 0);
}

#[test]
fn current_entity_power_decrease_only_matches_the_owners_spend() {
    let managers = BattleManagers::default();
    let pool = attacker_pool();
    let condition = condition(ParsedConditionKind::CurrentEntityPowerDecrease);
    let spent = ResourceEvent::Eureka {
        target_uid: 10,
        eureka_id: 1,
        applied_delta: -2,
        overflow: 0,
    };

    assert_eq!(
        event_conditions_count(
            std::slice::from_ref(&condition),
            context(spent, 10, &[10], &managers, &pool),
        ),
        1
    );
    assert_eq!(
        event_conditions_count(&[condition], context(spent, 11, &[11], &managers, &pool)),
        0
    );
}

#[test]
fn lost_power_counts_each_point_consumed_by_a_targeted_ally() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![10, 11]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    current_hp: Some(1),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let condition = condition(ParsedConditionKind::LostPower {
        power_id: 1,
        threshold: 1,
    });
    let event = |target_uid| ResourceEvent::Eureka {
        target_uid,
        eureka_id: 1,
        applied_delta: -2,
        overflow: 0,
    };
    let scoped = |target_uid| ResourceConditionContext {
        condition_target_code: 101,
        ..context(event(target_uid), 10, &[10, 11], &managers, &pool)
    };

    assert!(has_event_condition(std::slice::from_ref(&condition)));
    assert_eq!(
        event_conditions_count(std::slice::from_ref(&condition), scoped(11)),
        2
    );
    assert_eq!(event_conditions_count(&[condition], scoped(-1)), 0);
}

#[test]
fn power_use_add_buff_fires_once_when_the_carrier_crosses_the_round_threshold() {
    let mut managers = BattleManagers::default();
    managers.eureka.add_max(10, EUREKA_RESOURCE_ID, 4);
    managers.eureka.set(10, EUREKA_RESOURCE_ID, 4);
    let pool = attacker_pool();
    let condition = condition(ParsedConditionKind::PowerUseAddBuff { threshold: 2 });
    let event = |target_uid, applied_delta| ResourceEvent::Eureka {
        target_uid,
        eureka_id: EUREKA_RESOURCE_ID,
        applied_delta,
        overflow: 0,
    };

    managers.eureka.add(10, 10, EUREKA_RESOURCE_ID, -1, 0);
    assert_eq!(
        event_conditions_count(
            std::slice::from_ref(&condition),
            context(event(10, -1), 10, &[10], &managers, &pool),
        ),
        0
    );

    managers.eureka.add(10, 10, EUREKA_RESOURCE_ID, -1, 0);
    assert_eq!(
        event_conditions_count(
            std::slice::from_ref(&condition),
            context(event(10, -1), 10, &[10], &managers, &pool),
        ),
        1
    );

    managers.eureka.add(10, 10, EUREKA_RESOURCE_ID, -1, 0);
    assert_eq!(
        event_conditions_count(
            std::slice::from_ref(&condition),
            context(event(10, -1), 10, &[10], &managers, &pool),
        ),
        0
    );
}

#[test]
fn power_use_add_buff_fires_once_for_a_large_carrier_spend_and_never_for_an_ally() {
    let mut managers = BattleManagers::default();
    for uid in [10, 11] {
        managers.eureka.add_max(uid, EUREKA_RESOURCE_ID, 4);
        managers.eureka.set(uid, EUREKA_RESOURCE_ID, 4);
    }
    let pool = attacker_pool();
    let condition = condition(ParsedConditionKind::PowerUseAddBuff { threshold: 2 });
    let event = |target_uid, applied_delta| ResourceEvent::Eureka {
        target_uid,
        eureka_id: EUREKA_RESOURCE_ID,
        applied_delta,
        overflow: 0,
    };

    managers.eureka.add(11, 11, EUREKA_RESOURCE_ID, -2, 0);
    assert_eq!(
        event_conditions_count(
            std::slice::from_ref(&condition),
            context(event(11, -2), 10, &[10], &managers, &pool),
        ),
        0
    );

    managers.eureka.add(10, 10, EUREKA_RESOURCE_ID, -4, 0);
    assert_eq!(
        event_conditions_count(
            std::slice::from_ref(&condition),
            context(event(10, -4), 10, &[10], &managers, &pool),
        ),
        1
    );
}

#[test]
fn lost_ex_point_is_boolean_while_per_decrease_counts_points() {
    let managers = BattleManagers::default();
    let pool = attacker_pool();
    let lost = condition(ParsedConditionKind::ExPointLost);
    let per_point = condition(ParsedConditionKind::ExPointDecrease { threshold: 1 });
    let event = ResourceEvent::ExPoint {
        target_uid: 10,
        kind: 0,
        delta: -3,
    };

    assert_eq!(
        event_conditions_count(&[lost], context(event, 10, &[10], &managers, &pool)),
        1
    );
    assert_eq!(
        event_conditions_count(&[per_point], context(event, 10, &[10], &managers, &pool),),
        3
    );
}

#[test]
fn conduit_cost_keeps_owner_team_and_side_scopes_distinct() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![10, 11]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    current_hp: Some(1),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let owner = condition(ParsedConditionKind::PerConduitCurrentCost {
        threshold: 1,
        scope: ConduitActivationScope::Owner,
    });
    let team = condition(ParsedConditionKind::PerConduitCurrentCost {
        threshold: 1,
        scope: ConduitActivationScope::Team,
    });
    let event = |target_uid| ResourceEvent::Conduit {
        target_uid,
        power_id: 1,
        activation_cost: 3,
        spent: 2,
    };

    assert_eq!(
        event_conditions_count(
            std::slice::from_ref(&owner),
            context(event(10), 10, &[10], &managers, &pool),
        ),
        3
    );
    assert_eq!(
        event_conditions_count(
            std::slice::from_ref(&owner),
            context(event(11), 10, &[10], &managers, &pool),
        ),
        0
    );
    assert_eq!(
        event_conditions_count(
            std::slice::from_ref(&team),
            context(event(11), 10, &[10, 11], &managers, &pool),
        ),
        3
    );
    assert_eq!(
        event_conditions_count(
            std::slice::from_ref(&owner),
            context(
                event(10),
                crate::engine::fight::rules::ATTACKER_SIDE_UID,
                &[crate::engine::fight::rules::ATTACKER_SIDE_UID],
                &managers,
                &pool,
            ),
        ),
        3
    );
}
