use crate::engine::manager::buff::BuffStatus;

use super::*;

pub(super) fn buff_id(
    mode: &BuffConditionMode,
    buff_ids: &[i32],
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
) -> bool {
    let Some(managers) = managers else {
        return false;
    };
    if condition_targets.is_empty() {
        return false;
    }

    match mode {
        BuffConditionMode::Present | BuffConditionMode::PresentAndConsume => {
            condition_targets.iter().any(|target_uid| {
                buff_ids.iter().any(|buff_id| {
                    managers
                        .buff
                        .has_active_buff_id_or_type(*target_uid, *buff_id)
                })
            })
        }
        BuffConditionMode::Absent => condition_targets.iter().all(|target_uid| {
            !buff_ids.iter().any(|buff_id| {
                managers
                    .buff
                    .has_active_buff_id_or_type(*target_uid, *buff_id)
            })
        }),
        BuffConditionMode::ExactPresent => condition_targets.iter().any(|target_uid| {
            buff_ids
                .iter()
                .any(|buff_id| managers.buff.has_active_buff_id(*target_uid, *buff_id))
        }),
        BuffConditionMode::ExactAbsent => condition_targets.iter().all(|target_uid| {
            !buff_ids
                .iter()
                .any(|buff_id| managers.buff.has_active_buff_id(*target_uid, *buff_id))
        }),
    }
}

pub(super) fn buff_id_count(
    buff_ids: &[i32],
    compare: &ConditionCompare,
    threshold: &i32,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
) -> bool {
    let Some(managers) = managers else {
        return false;
    };
    let amount: i32 = condition_targets
        .iter()
        .map(|uid| {
            buff_ids
                .iter()
                .map(|buff_id| managers.buff.buff_id_or_type_amount(*uid, *buff_id))
                .sum::<i32>()
        })
        .sum();
    compare_value(amount, *compare, *threshold)
}

pub(super) fn buff_id_threshold(
    buff_ids: &[i32],
    threshold: &i32,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
) -> bool {
    managers.is_some_and(|managers| {
        condition_targets
            .iter()
            .map(|uid| {
                buff_ids
                    .iter()
                    .map(|buff_id| managers.buff.buff_id_or_type_amount(*uid, *buff_id))
                    .sum::<i32>()
            })
            .sum::<i32>()
            >= *threshold
    })
}

pub(super) fn player_buff_presence(
    present: &bool,
    buff_id: &i32,
    managers: Option<&BattleManagers>,
) -> bool {
    managers.is_some_and(|managers| {
        managers
            .buff
            .has_active_buff_id_or_type(crate::engine::fight::rules::ATTACKER_SIDE_UID, *buff_id)
            == *present
    })
}

pub(super) fn buff_type_count(
    type_ids: &[i32],
    compare: &ConditionCompare,
    threshold: &i32,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
) -> bool {
    let Some(managers) = managers else {
        return false;
    };
    let amount: i32 = condition_targets
        .iter()
        .map(|uid| {
            type_ids
                .iter()
                .map(|type_id| managers.buff.buff_type_amount(*uid, *type_id))
                .sum::<i32>()
        })
        .sum();
    compare_value(amount, *compare, *threshold)
}

pub(super) fn any_target_buff_type_count(
    type_ids: &[i32],
    threshold: &i32,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
) -> bool {
    managers.is_some_and(|managers| {
        condition_targets.iter().any(|uid| {
            type_ids
                .iter()
                .map(|type_id| managers.buff.buff_type_amount(*uid, *type_id))
                .sum::<i32>()
                >= *threshold
        })
    })
}

pub(super) fn all_target_buff_type_count(
    type_ids: &[i32],
    threshold: &i32,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
) -> bool {
    managers.is_some_and(|managers| {
        !condition_targets.is_empty()
            && condition_targets.iter().all(|uid| {
                type_ids
                    .iter()
                    .map(|type_id| managers.buff.buff_type_amount(*uid, *type_id))
                    .sum::<i32>()
                    >= *threshold
            })
    })
}

pub(super) fn from_buff_and_to_buff(
    from_buff_id: &i32,
    to_buff_id: &i32,
    source_uid: i64,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
) -> bool {
    managers.is_some_and(|managers| {
        // The hit comes from a condition target holding `from` to the owner holding `to`.
        managers
            .buff
            .has_active_buff_id_or_type(source_uid, *to_buff_id)
            && condition_targets.iter().any(|target_uid| {
                managers
                    .buff
                    .has_active_buff_id_or_type(*target_uid, *from_buff_id)
            })
    })
}

pub(super) fn self_buff_type_target_buff_types(
    self_type_id: &i32,
    target_type_ids: &[i32],
    source_uid: i64,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
    context: TargetContext,
) -> bool {
    managers.is_some_and(|managers| {
        let target_uid = context.active_skill_source_uid;
        managers.buff.buff_type_amount(source_uid, *self_type_id) > 0
            && target_uid != 0
            && condition_targets.contains(&target_uid)
            && target_type_ids
                .iter()
                .any(|type_id| managers.buff.buff_type_amount(target_uid, *type_id) > 0)
    })
}

pub(super) fn enemy_highest_buff_type_count(
    type_id: &i32,
    threshold: &i32,
    source_uid: i64,
    managers: Option<&BattleManagers>,
    pool: &TargetPool,
) -> bool {
    managers.is_some_and(|managers| {
        pool.enemies(source_uid, true)
            .iter()
            .map(|enemy| managers.buff.buff_type_amount(enemy.uid, *type_id))
            .max()
            .unwrap_or_default()
            >= *threshold
    })
}

pub(super) fn per_buff_type_layer(
    type_ids: &[i32],
    layers_per_repeat: &i32,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
) -> bool {
    managers
        .and_then(|managers| {
            super::super::buff::holder_layers(type_ids, condition_targets, managers)
        })
        .is_some_and(|count| count >= *layers_per_repeat)
}

pub(super) fn buff_status_count(
    status_ids: &[i32],
    compare: &ConditionCompare,
    threshold: &i32,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
) -> bool {
    managers.is_some_and(|managers| {
        let amount = condition_targets
            .iter()
            .map(|uid| managers.buff.buff_status_count(*uid, status_ids))
            .sum();
        compare_value(amount, *compare, *threshold)
    })
}

pub(super) fn primary_status_absent(
    statuses: &[BuffStatus],
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
) -> bool {
    managers.is_some_and(|managers| {
        condition_targets.iter().all(|uid| {
            statuses
                .iter()
                .all(|status| !managers.buff.has_active_status(*uid, *status))
        })
    })
}

pub(super) fn visible_buff_status_type_count(
    status_ids: &[i32],
    compare: &ConditionCompare,
    threshold: &i32,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
) -> bool {
    managers.is_some_and(|managers| {
        condition_targets.iter().any(|uid| {
            let amount = managers
                .buff
                .visible_buff_status_type_count(*uid, status_ids);
            compare_value(amount, *compare, *threshold)
        })
    })
}

pub(super) fn visible_stacked_status_type_count(
    status_ids: &[i32],
    compare: &ConditionCompare,
    threshold: &i32,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
) -> bool {
    managers.is_some_and(|managers| {
        condition_targets.iter().any(|uid| {
            let amount = managers
                .buff
                .visible_stacked_status_type_count(*uid, status_ids);
            compare_value(amount, *compare, *threshold)
        })
    })
}

pub(super) fn per_team_buff_status_type_count(
    status_ids: &[i32],
    divisor: &i32,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
) -> bool {
    managers.is_some_and(|managers| {
        *divisor > 0
            && managers
                .buff
                .team_buff_status_type_count(condition_targets, status_ids)
                >= *divisor
    })
}

pub(super) fn buff_added(
    buff_ids: &[i32],
    condition_targets: &[i64],
    context: TargetContext,
) -> bool {
    context.added_buff_amount > 0
        && condition_targets.contains(&context.added_buff_target_uid)
        && (buff_ids.contains(&context.added_buff_id)
            || buff_ids.contains(&context.added_buff_type_id))
}

pub(super) fn buff_type_added(
    type_ids: &[i32],
    condition_targets: &[i64],
    context: TargetContext,
) -> bool {
    context.added_buff_amount > 0
        && condition_targets.contains(&context.added_buff_target_uid)
        && type_ids.contains(&context.added_buff_status_id)
}

pub(super) fn buff_removed(
    buff_ids: &[i32],
    condition_targets: &[i64],
    context: TargetContext,
) -> bool {
    context.removed_buff_target_uid != 0
        && condition_targets.contains(&context.removed_buff_target_uid)
        && buff_ids.contains(&context.removed_buff_id)
}

pub(super) fn buff_expired(
    buff_ids: &[i32],
    condition_targets: &[i64],
    context: TargetContext,
) -> bool {
    context.removed_buff_duration_expired
        && condition_targets.contains(&context.removed_buff_target_uid)
        && buff_ids.contains(&context.removed_buff_id)
}
