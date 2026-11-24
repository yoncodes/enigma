use super::*;

pub(super) fn hp_permille(
    compare: &ConditionCompare,
    threshold: &i32,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
) -> bool {
    let Some(managers) = managers else {
        return false;
    };
    condition_targets.iter().any(|uid| {
        let max = managers.hp.max(*uid);
        max > 0
            && compare_value(
                ((managers.hp.current(*uid) as i64 * 1000) / max as i64) as i32,
                *compare,
                *threshold,
            )
    })
}

pub(super) fn per_hp(
    interval_permille: &i32,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
) -> bool {
    managers.is_some_and(|managers| {
        *interval_permille > 0
            && condition_targets
                .iter()
                .any(|uid| managers.hp.current(*uid) > 0)
    })
}

pub(super) fn per_lost_hp(
    interval_permille: &i32,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
    context: TargetContext,
) -> bool {
    managers.is_some_and(|managers| {
        let hit_uid = (context.hit_target_uid != 0
            && condition_targets.contains(&context.hit_target_uid))
        .then_some(context.hit_target_uid);
        hit_uid.map_or_else(
            || {
                condition_targets.iter().any(|uid| {
                    super::super::hp::lost_hp_interval_count(*uid, *interval_permille, managers) > 0
                })
            },
            |uid| super::super::hp::lost_hp_interval_count(uid, *interval_permille, managers) > 0,
        )
    })
}

pub(super) fn team_lost_hp_percent(
    team_type: &i32,
    interval_permille: &i32,
    max_count: &i32,
    managers: Option<&BattleManagers>,
    pool: &TargetPool,
) -> bool {
    managers.is_some_and(|managers| {
        super::super::hp::team_lost_hp_count(
            *team_type,
            *interval_permille,
            *max_count,
            managers,
            pool,
        ) > 0
    })
}

pub(super) fn blood_pool_value(
    min: &i32,
    max: &i32,
    config_effect: &i32,
    context: TargetContext,
) -> bool {
    let value = match config_effect {
        0 => context.blood_pool_value,
        1 => context.heat_scale_raw_value,
        _ => return false,
    };
    (*min..=*max).contains(&value)
}

pub(super) fn ex_point(
    compare: &ConditionCompare,
    threshold: &i32,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
) -> bool {
    let Some(managers) = managers else {
        return false;
    };
    condition_targets
        .iter()
        .any(|uid| compare_value(managers.ex_point.get(*uid), *compare, *threshold))
}

pub(super) fn ex_point_full(condition_targets: &[i64], managers: Option<&BattleManagers>) -> bool {
    managers.is_some_and(|managers| {
        condition_targets
            .iter()
            .any(|uid| managers.ex_point.is_full(*uid))
    })
}

pub(super) fn synchronization(
    threshold: &i32,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
) -> bool {
    let Some(managers) = managers else {
        return false;
    };
    condition_targets.iter().any(|uid| {
        managers.ex_point.kind(*uid)
            == crate::engine::manager::ex_point::ExPointKind::Synchronization.as_wire()
            && managers.ex_point.get(*uid) >= *threshold
    })
}

pub(super) fn per_ex_point(
    threshold: &i32,
    source_uid: i64,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
    context: TargetContext,
) -> bool {
    managers.is_some_and(|managers| {
        condition_targets.iter().any(|uid| {
            if *uid == source_uid {
                context
                    .action_start_ex_point
                    .unwrap_or_else(|| managers.ex_point.get(*uid))
                    >= *threshold
            } else {
                managers.ex_point.get(*uid) >= *threshold
            }
        })
    })
}

pub(super) fn per_eureka(source_uid: i64, managers: Option<&BattleManagers>) -> bool {
    managers.is_some_and(|managers| {
        managers
            .eureka
            .get(
                source_uid,
                crate::engine::manager::eureka::EUREKA_RESOURCE_ID,
            )
            .current
            > 0
    })
}

pub(super) fn ex_point_decrease(
    threshold: &i32,
    condition_targets: &[i64],
    context: TargetContext,
) -> bool {
    context.ex_point_changed_uid != 0
        && context.ex_point_delta < 0
        && -context.ex_point_delta >= *threshold
        && condition_targets.contains(&context.ex_point_changed_uid)
}

pub(super) fn ex_point_lost(condition_targets: &[i64], context: TargetContext) -> bool {
    context.ex_point_changed_uid != 0
        && context.ex_point_delta < 0
        && condition_targets.contains(&context.ex_point_changed_uid)
}

pub(super) fn power_compare(
    compare_code: &i32,
    power_id: &i32,
    threshold: &i32,
    source_uid: i64,
    managers: Option<&BattleManagers>,
) -> bool {
    let Some(managers) = managers else {
        return false;
    };
    compare_resource(
        managers.eureka.get(source_uid, *power_id).current,
        *compare_code,
        *threshold,
    )
}

pub(super) fn power_ratio(
    power_id: &i32,
    compare_code: &i32,
    threshold_permille: &i32,
    source_uid: i64,
    managers: Option<&BattleManagers>,
) -> bool {
    managers.is_some_and(|managers| {
        let power = managers.eureka.get(source_uid, *power_id);
        power.max > 0
            && compare_resource(
                ((power.current as i64 * 1000) / power.max as i64) as i32,
                *compare_code,
                *threshold_permille,
            )
    })
}

pub(super) fn conduit_ex_point(
    compare_code: &i32,
    threshold: &i32,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
) -> bool {
    managers.is_some_and(|managers| {
        condition_targets
            .iter()
            .any(|uid| compare_resource(managers.ex_point.get(*uid), *compare_code, *threshold))
    })
}

pub(super) fn conduit_skill_group(
    group: &i32,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
) -> bool {
    managers.is_some_and(|managers| {
        condition_targets
            .iter()
            .any(|uid| managers.conduit.selected_group(*uid) == Some(*group))
    })
}

pub(super) fn power_overflow(
    power_id: &i32,
    max_count: &i32,
    source_uid: i64,
    managers: Option<&BattleManagers>,
) -> bool {
    managers.is_some_and(|managers| {
        managers.eureka.round_change(source_uid, *power_id).overflow > 0 && *max_count > 0
    })
}

pub(super) fn power_consumed(
    power_id: &i32,
    max_count: &i32,
    source_uid: i64,
    managers: Option<&BattleManagers>,
) -> bool {
    managers.is_some_and(|managers| {
        managers.eureka.round_change(source_uid, *power_id).consumed > 0 && *max_count > 0
    })
}

pub(super) fn lost_power(power_id: &i32, threshold: &i32, context: TargetContext) -> bool {
    context.lost_power_id == *power_id && context.lost_power_amount >= *threshold
}
