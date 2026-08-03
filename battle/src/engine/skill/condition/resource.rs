use crate::engine::{
    manager::{BattleManagers, eureka::EUREKA_RESOURCE_ID},
    skill::{
        condition::parse::{
            ConditionCompare, ExPointIncreaseScope, first_i32, parse_fixed, parse_i32,
        },
        condition::{ParsedCondition, ParsedConditionKind, conditions_match},
        target::{TargetContext, TargetPool},
    },
};

pub fn ex_point_at_least(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    ex_point(args, ConditionCompare::GreaterThanOrEqual)
}

pub fn ex_point_at_most(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    ex_point(args, ConditionCompare::LessThanOrEqual)
}

fn ex_point(args: &[String], compare: ConditionCompare) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::ExPoint {
        compare,
        threshold: first_i32(args)?,
    })
}

pub fn synchronization(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::Synchronization {
        threshold: first_i32(args)?,
    })
}

pub fn per_ex_point(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::PerExPoint {
        threshold: first_i32(args)?,
    })
}

pub fn ex_point_decrease(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::ExPointDecrease {
        threshold: first_i32(args)?,
    })
}

pub fn ex_point_lost(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    (first_i32(args)? == 0).then_some(ParsedConditionKind::ExPointLost)
}

pub fn power_use_add_buff(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::PowerUseAddBuff {
        threshold: first_i32(args)?,
    })
}

pub fn blood_pool_max(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::BloodPoolMax {
        min: first_i32(args)?,
        max: args
            .get(1)
            .and_then(|arg| parse_i32(arg))
            .unwrap_or(i32::MAX),
    })
}

pub fn blood_pool_value(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::BloodPoolValue {
        min: first_i32(args)?,
        max: args.get(1).and_then(|arg| parse_i32(arg))?,
        config_effect: args
            .get(2)
            .and_then(|arg| parse_i32(arg))
            .unwrap_or_default(),
    })
}

fn ex_point_increase(args: &[String], scope: ExPointIncreaseScope) -> Option<ParsedConditionKind> {
    let [threshold, kind] = parse_fixed(args)?;
    Some(ParsedConditionKind::ExPointIncrChange {
        threshold,
        kind,
        scope,
    })
}

pub fn self_ex_point_increase(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    ex_point_increase(args, ExPointIncreaseScope::SelfOnly)
}

pub fn other_ally_ex_point_increase(
    _: i32,
    _: &str,
    args: &[String],
) -> Option<ParsedConditionKind> {
    ex_point_increase(args, ExPointIncreaseScope::OtherAlly)
}

pub fn power_compare(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [compare_code, power_id, threshold] = parse_fixed(args)?;
    Some(ParsedConditionKind::PowerCompare {
        compare_code,
        power_id,
        threshold,
    })
}

pub fn power_ratio(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [power_id, compare_code, threshold_permille] = parse_fixed(args)?;
    Some(ParsedConditionKind::PowerRatio {
        power_id,
        compare_code,
        threshold_permille,
    })
}

pub fn power_increase(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [power_id, compare_code, threshold] = parse_fixed(args)?;
    Some(ParsedConditionKind::PowerIncrChange {
        power_id,
        compare_code,
        threshold,
    })
}

pub fn power_overflow(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [power_id, max_count] = parse_fixed(args)?;
    Some(ParsedConditionKind::PowerOverflow {
        power_id,
        max_count,
    })
}

pub fn power_consumed(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [power_id, max_count] = parse_fixed(args)?;
    Some(ParsedConditionKind::PowerConsumed {
        power_id,
        max_count,
    })
}

pub fn per_conduit_current_cost(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::PerConduitCurrentCost {
        threshold: first_i32(args)?.max(1),
    })
}

pub fn current_entity_power_decrease(_: i32, _: &str, _: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::CurrentEntityPowerDecrease)
}

pub fn lost_power(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [power_id, threshold] = parse_fixed(args)?;
    Some(ParsedConditionKind::LostPower {
        power_id,
        threshold,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourceEvent {
    ExPoint {
        target_uid: i64,
        kind: i32,
        delta: i32,
    },
    Eureka {
        target_uid: i64,
        eureka_id: i32,
        applied_delta: i32,
        overflow: i32,
    },
    Conduit {
        target_uid: i64,
        power_id: i32,
        activation_cost: i32,
        spent: i32,
    },
}

#[derive(Clone, Copy)]
pub(crate) struct ResourceConditionContext<'a> {
    pub event: ResourceEvent,
    pub source_uid: i64,
    pub condition_targets: &'a [i64],
    pub condition_target_code: i32,
    pub managers: &'a BattleManagers,
    pub pool: &'a TargetPool,
    pub random_roll: Option<i32>,
}

pub(crate) fn has_event_condition(conditions: &[ParsedCondition]) -> bool {
    conditions.iter().any(|condition| match &condition.kind {
        ParsedConditionKind::Any(groups) => groups.iter().any(|group| has_event_condition(group)),
        ParsedConditionKind::Not(inner) => has_event_kind(inner),
        ParsedConditionKind::ExPointDecrease { .. }
        | ParsedConditionKind::ExPointLost
        | ParsedConditionKind::ExPointIncrChange { .. }
        | ParsedConditionKind::PowerIncrChange { .. }
        | ParsedConditionKind::PowerOverflow { .. }
        | ParsedConditionKind::PowerConsumed { .. }
        | ParsedConditionKind::PerConduitCurrentCost { .. }
        | ParsedConditionKind::CurrentEntityPowerDecrease
        | ParsedConditionKind::PowerUseAddBuff { .. }
        | ParsedConditionKind::LostPower { .. } => true,
        _ => false,
    })
}

fn has_event_kind(kind: &ParsedConditionKind) -> bool {
    match kind {
        ParsedConditionKind::Not(inner) => has_event_kind(inner),
        ParsedConditionKind::ExPointDecrease { .. }
        | ParsedConditionKind::ExPointLost
        | ParsedConditionKind::ExPointIncrChange { .. }
        | ParsedConditionKind::PowerIncrChange { .. }
        | ParsedConditionKind::PowerOverflow { .. }
        | ParsedConditionKind::PowerConsumed { .. }
        | ParsedConditionKind::PerConduitCurrentCost { .. }
        | ParsedConditionKind::CurrentEntityPowerDecrease
        | ParsedConditionKind::PowerUseAddBuff { .. }
        | ParsedConditionKind::LostPower { .. } => true,
        _ => false,
    }
}

pub(crate) fn event_conditions_count(
    conditions: &[ParsedCondition],
    context: ResourceConditionContext<'_>,
) -> i32 {
    conditions
        .iter()
        .map(|condition| event_condition_count(condition, context))
        .try_fold(i32::MAX, |min, count| (count > 0).then_some(min.min(count)))
        .unwrap_or(0)
}

fn event_condition_count(
    condition: &ParsedCondition,
    context: ResourceConditionContext<'_>,
) -> i32 {
    let ResourceConditionContext {
        event,
        source_uid,
        condition_targets,
        condition_target_code,
        managers,
        pool,
        random_roll,
    } = context;
    match &condition.kind {
        ParsedConditionKind::Any(groups) => groups
            .iter()
            .map(|group| event_conditions_count(group, context))
            .max()
            .unwrap_or(0),
        ParsedConditionKind::ExPointIncrChange {
            threshold,
            kind,
            scope,
        } => {
            let ResourceEvent::ExPoint {
                target_uid,
                kind: event_kind,
                delta,
            } = event
            else {
                return 0;
            };
            if delta > 0
                && event_kind == *kind
                && super::ex_point::increase_in_scope(
                    target_uid,
                    source_uid,
                    *scope,
                    pool.team_type(source_uid)
                        .is_some_and(|team| pool.team_type(target_uid) == Some(team)),
                )
            {
                delta / (*threshold).max(1)
            } else {
                0
            }
        }
        ParsedConditionKind::ExPointDecrease { threshold } => {
            let ResourceEvent::ExPoint {
                target_uid, delta, ..
            } = event
            else {
                return 0;
            };
            if condition_targets.contains(&target_uid) {
                super::ex_point::decrease_count(delta, *threshold)
            } else {
                0
            }
        }
        ParsedConditionKind::ExPointLost => {
            let ResourceEvent::ExPoint {
                target_uid, delta, ..
            } = event
            else {
                return 0;
            };
            i32::from(delta < 0 && condition_targets.contains(&target_uid))
        }
        ParsedConditionKind::PowerIncrChange {
            power_id,
            compare_code,
            threshold,
        } => {
            let ResourceEvent::Eureka {
                target_uid,
                eureka_id,
                applied_delta,
                ..
            } = event
            else {
                return 0;
            };
            i32::from(
                eureka_id == *power_id
                    && super::evaluate::compare_resource(applied_delta, *compare_code, *threshold)
                    && event_in_scope(
                        target_uid,
                        source_uid,
                        condition_targets,
                        condition_target_code,
                        pool,
                    ),
            )
        }
        ParsedConditionKind::PowerOverflow {
            power_id,
            max_count,
        } => {
            let ResourceEvent::Eureka {
                target_uid,
                eureka_id,
                overflow,
                ..
            } = event
            else {
                return 0;
            };
            if eureka_id == *power_id
                && event_in_scope(
                    target_uid,
                    source_uid,
                    condition_targets,
                    condition_target_code,
                    pool,
                )
            {
                overflow.min((*max_count).max(0))
            } else {
                0
            }
        }
        ParsedConditionKind::PowerConsumed {
            power_id,
            max_count,
        } => {
            let ResourceEvent::Eureka {
                target_uid,
                eureka_id,
                applied_delta,
                ..
            } = event
            else {
                return 0;
            };
            if eureka_id == *power_id
                && event_in_scope(
                    target_uid,
                    source_uid,
                    condition_targets,
                    condition_target_code,
                    pool,
                )
            {
                (-applied_delta).max(0).min((*max_count).max(0))
            } else {
                0
            }
        }
        ParsedConditionKind::PerConduitCurrentCost { threshold } => {
            let ResourceEvent::Conduit {
                target_uid,
                activation_cost,
                ..
            } = event
            else {
                return 0;
            };
            if event_in_scope(
                target_uid,
                source_uid,
                condition_targets,
                condition_target_code,
                pool,
            ) {
                activation_cost.max(0) / (*threshold).max(1)
            } else {
                0
            }
        }
        ParsedConditionKind::CurrentEntityPowerDecrease => {
            let ResourceEvent::Eureka {
                target_uid,
                applied_delta,
                ..
            } = event
            else {
                return 0;
            };
            i32::from(target_uid == source_uid && applied_delta < 0)
        }
        ParsedConditionKind::PowerUseAddBuff { threshold } => {
            let ResourceEvent::Eureka {
                target_uid,
                eureka_id,
                applied_delta,
                ..
            } = event
            else {
                return 0;
            };
            if target_uid != source_uid || eureka_id != EUREKA_RESOURCE_ID || applied_delta >= 0 {
                return 0;
            }

            let threshold = (*threshold).max(1);
            let consumed = managers
                .eureka
                .round_change(source_uid, EUREKA_RESOURCE_ID)
                .consumed;
            let consumed_before_event = consumed.saturating_sub(-applied_delta);
            i32::from(consumed_before_event < threshold && consumed >= threshold)
        }
        ParsedConditionKind::LostPower {
            power_id,
            threshold,
        } => {
            let ResourceEvent::Eureka {
                target_uid,
                eureka_id,
                applied_delta,
                ..
            } = event
            else {
                return 0;
            };
            if eureka_id == *power_id
                && applied_delta < 0
                && event_in_scope(
                    target_uid,
                    source_uid,
                    condition_targets,
                    condition_target_code,
                    pool,
                )
            {
                -applied_delta / (*threshold).max(1)
            } else {
                0
            }
        }
        ParsedConditionKind::Random { threshold } => {
            random_roll.map_or(1, |roll| i32::from(roll < *threshold))
        }
        ParsedConditionKind::AccBuffAddedCount { .. } => 0,
        _ => i32::from(conditions_match(
            std::slice::from_ref(condition),
            source_uid,
            condition_targets,
            Some(managers),
            pool,
            TargetContext::default(),
        )),
    }
}

fn event_in_scope(
    event_target_uid: i64,
    source_uid: i64,
    condition_targets: &[i64],
    condition_target_code: i32,
    pool: &TargetPool,
) -> bool {
    if condition_target_code == 103 {
        pool.team_type(source_uid)
            .is_some_and(|team| pool.team_type(event_target_uid) == Some(team))
    } else {
        condition_targets.contains(&event_target_uid)
    }
}

#[cfg(test)]
mod tests {
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
    fn conduit_cost_counts_each_consumed_energy_unit() {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);
        let condition = condition(ParsedConditionKind::PerConduitCurrentCost { threshold: 1 });

        assert_eq!(
            event_conditions_count(
                &[condition],
                context(
                    ResourceEvent::Conduit {
                        target_uid: 10,
                        power_id: 1,
                        activation_cost: 3,
                        spent: 2,
                    },
                    10,
                    &[10],
                    &managers,
                    &pool,
                ),
            ),
            3
        );
    }
}
