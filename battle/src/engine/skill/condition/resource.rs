use crate::engine::{
    manager::{BattleManagers, eureka::EUREKA_RESOURCE_ID},
    skill::{
        condition::parse::{
            ConditionCompare, ConduitActivationScope, ExPointIncreaseScope, first_i32, parse_fixed,
            parse_i32,
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

pub fn ex_point_full(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    args.is_empty().then_some(ParsedConditionKind::ExPointFull)
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

pub fn per_eureka(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    args.is_empty().then_some(ParsedConditionKind::PerEureka)
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

pub fn positive_power_compare(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [compare_code, power_id, threshold] = parse_fixed(args)?;
    (matches!(compare_code, 1..=3) && power_id > 0 && threshold >= 0).then_some(
        ParsedConditionKind::PowerCompare {
            compare_code,
            power_id,
            threshold,
        },
    )
}

pub fn power_ratio(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [power_id, compare_code, threshold_permille] = parse_fixed(args)?;
    Some(ParsedConditionKind::PowerRatio {
        power_id,
        compare_code,
        threshold_permille,
    })
}

pub fn fight_progress_value(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [progress_id, compare_code, threshold] = parse_fixed(args)?;
    if !matches!(compare_code, 1..=3) {
        return None;
    }
    Some(ParsedConditionKind::FightProgressValue {
        progress_id,
        compare_code,
        threshold,
    })
}

pub fn progress_complete(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [progress_id, compare_code, threshold] = parse_fixed(args)?;
    (progress_id == 8 && compare_code == 3 && threshold == 100).then_some(
        ParsedConditionKind::FightProgressValue {
            progress_id,
            compare_code,
            threshold,
        },
    )
}

pub fn progress_band(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [progress_id, compare_code, threshold] = parse_fixed(args)?;
    matches!(
        (progress_id, compare_code, threshold),
        (8, 1, 0 | 30 | 60) | (8, 2, 29 | 59 | 99)
    )
    .then_some(ParsedConditionKind::FightProgressValue {
        progress_id,
        compare_code,
        threshold,
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

fn per_conduit_current_cost(
    args: &[String],
    scope: ConduitActivationScope,
) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::PerConduitCurrentCost {
        threshold: first_i32(args)?.max(1),
        scope,
    })
}

pub fn owner_conduit_current_cost(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    per_conduit_current_cost(args, ConduitActivationScope::Owner)
}

pub fn team_conduit_current_cost(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    per_conduit_current_cost(args, ConduitActivationScope::Team)
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
        ParsedConditionKind::PerConduitCurrentCost { threshold, scope } => {
            let ResourceEvent::Conduit {
                target_uid,
                activation_cost,
                ..
            } = event
            else {
                return 0;
            };
            let in_scope = match scope {
                ConduitActivationScope::Owner
                    if crate::engine::fight::rules::is_side_uid(source_uid) =>
                {
                    pool.team_type(source_uid)
                        .is_some_and(|team| pool.team_type(target_uid) == Some(team))
                }
                ConduitActivationScope::Owner => target_uid == source_uid,
                ConduitActivationScope::Team => event_in_scope(
                    target_uid,
                    source_uid,
                    condition_targets,
                    condition_target_code,
                    pool,
                ),
            };
            if in_scope {
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
        ParsedConditionKind::AccBuffAddedCount { .. }
        | ParsedConditionKind::AccBuffTypeAddedCount { .. } => 0,
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
mod tests;
