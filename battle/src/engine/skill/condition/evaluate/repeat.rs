use super::*;

pub(super) fn condition_repeat_count(
    condition: &ParsedCondition,
    source_uid: i64,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
    pool: &TargetPool,
    context: TargetContext,
) -> Option<i32> {
    match &condition.kind {
        ParsedConditionKind::Any(groups) => groups
            .iter()
            .filter(|group| {
                conditions_match(
                    group,
                    source_uid,
                    condition_targets,
                    managers,
                    pool,
                    context,
                )
            })
            .map(|group| {
                group
                    .iter()
                    .filter_map(|condition| {
                        condition_repeat_count(
                            condition,
                            source_uid,
                            condition_targets,
                            managers,
                            pool,
                            context,
                        )
                    })
                    .min()
                    .unwrap_or(1)
            })
            .max(),
        ParsedConditionKind::PerTargetCareerCount { careers, threshold } => Some(
            per_target_career_count(source_uid, careers, *threshold, pool),
        ),
        ParsedConditionKind::PerTeamCareerCount {
            career,
            divisor,
            opposing_team,
        } => Some(
            per_team_career_count(source_uid, *career, *opposing_team, pool) / (*divisor).max(1),
        ),
        ParsedConditionKind::OtherAllyDamageTypeCount {
            damage_type,
            max_count,
        } => Some(other_ally_damage_type_count(
            source_uid,
            *damage_type,
            *max_count,
            pool,
        )),
        ParsedConditionKind::TeamInjuryCountRound { max_count } => {
            Some(context.team_injury_count_round.clamp(0, *max_count))
        }
        ParsedConditionKind::PerKillCount { divisor } => {
            Some(context.action_kill_count / (*divisor).max(1))
        }
        ParsedConditionKind::ShareDamage => Some(context.damage_shared_count.max(1)),
        ParsedConditionKind::PerTeamBuffStatusTypeCount {
            status_ids,
            divisor,
            max_count,
        } => managers.map(|managers| {
            (managers
                .buff
                .team_buff_status_type_count(condition_targets, status_ids)
                / (*divisor).max(1))
            .clamp(0, *max_count)
        }),
        ParsedConditionKind::PerConduitCounter {
            kind,
            divisor,
            max_count,
        } => managers.map(|managers| {
            conduit_counter_count(source_uid, *kind, *divisor, *max_count, managers, pool)
        }),
        ParsedConditionKind::PerBuffGroupCount { group_id } => managers.map(|managers| {
            condition_targets
                .iter()
                .map(|uid| managers.buff.buff_group_amount(*uid, *group_id))
                .sum()
        }),
        ParsedConditionKind::PerBullet { divisor, max_count } => managers.map(|managers| {
            (managers.buff.buff_act_type_count(
                condition_targets,
                crate::engine::skill::buff_act::registry::BuffActKind::Bullet,
            ) / *divisor)
                .clamp(0, *max_count)
        }),
        ParsedConditionKind::PerHp { interval_permille } => managers.map(|managers| {
            condition_targets
                .iter()
                .map(|uid| {
                    let max = managers.hp.max(*uid);
                    if max <= 0 {
                        return 0;
                    }
                    let current_permille =
                        (managers.hp.current(*uid).max(0) as i64 * 1000 / max as i64) as i32;
                    current_permille / (*interval_permille).max(1)
                })
                .min()
                .unwrap_or_default()
        }),
        ParsedConditionKind::PerLostHp { interval_permille } => managers.map(|managers| {
            if context.hit_target_uid != 0 && condition_targets.contains(&context.hit_target_uid) {
                return super::super::hp::lost_hp_interval_count(
                    context.hit_target_uid,
                    *interval_permille,
                    managers,
                );
            }
            condition_targets
                .iter()
                .map(|uid| {
                    super::super::hp::lost_hp_interval_count(*uid, *interval_permille, managers)
                })
                .min()
                .unwrap_or_default()
        }),
        ParsedConditionKind::TeamLostHpPercent {
            team_type,
            interval_permille,
            max_count,
        } => managers.map(|managers| {
            super::super::hp::team_lost_hp_count(
                *team_type,
                *interval_permille,
                *max_count,
                managers,
                pool,
            )
        }),
        ParsedConditionKind::PerExPoint { threshold } => Some(
            managers
                .and_then(|managers| {
                    super::super::ex_point::per_ex_point_count(
                        *threshold,
                        context
                            .action_start_ex_point
                            .unwrap_or_else(|| managers.ex_point.get(source_uid)),
                    )
                })
                .unwrap_or_default(),
        ),
        ParsedConditionKind::PerEureka => managers.map(|managers| {
            managers
                .eureka
                .get(
                    source_uid,
                    crate::engine::manager::eureka::EUREKA_RESOURCE_ID,
                )
                .current
        }),
        ParsedConditionKind::PowerConsumed {
            power_id,
            max_count,
        } => Some(
            managers
                .map(|managers| {
                    managers
                        .eureka
                        .round_change(source_uid, *power_id)
                        .consumed
                        .min((*max_count).max(0))
                })
                .unwrap_or_default(),
        ),
        ParsedConditionKind::PowerOverflow {
            power_id,
            max_count,
        } => Some(
            managers
                .map(|managers| {
                    managers
                        .eureka
                        .round_change(source_uid, *power_id)
                        .overflow
                        .min((*max_count).max(0))
                })
                .unwrap_or_default(),
        ),
        ParsedConditionKind::PerBuffTypeLayer { .. } => Some(
            managers
                .map(|managers| {
                    super::super::buff::per_type_layer_count(
                        std::slice::from_ref(condition),
                        condition_targets,
                        managers,
                    )
                })
                .unwrap_or_default(),
        ),
        ParsedConditionKind::BuffIdCount {
            buff_ids,
            threshold,
            ..
        } => Some(
            managers
                .map(|managers| {
                    condition_targets
                        .iter()
                        .map(|uid| {
                            buff_ids
                                .iter()
                                .map(|buff_id| managers.buff.buff_id_or_type_amount(*uid, *buff_id))
                                .sum::<i32>()
                        })
                        .sum::<i32>()
                        / (*threshold).max(1)
                })
                .unwrap_or_default(),
        ),
        ParsedConditionKind::AccBuffAddedCount { .. } => Some(
            managers
                .map(|managers| {
                    super::super::buff::added_count_repeats(
                        condition, source_uid, managers, context,
                    )
                })
                .unwrap_or_default(),
        ),
        ParsedConditionKind::AccBuffTypeAddedCount { .. } => Some(
            managers
                .map(|managers| {
                    super::super::buff::added_type_count_repeats(condition, managers, context)
                })
                .unwrap_or_default(),
        ),
        ParsedConditionKind::BurnOverflow => Some(context.buff_overflow_amount),
        _ => None,
    }
}
