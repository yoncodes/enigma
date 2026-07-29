use crate::engine::{
    manager::BattleManagers,
    skill::{
        condition::{
            ConditionCompare, EntityCountScope, ParsedCondition, ParsedConditionKind,
            TargetIdentityMode, buff::BuffConditionMode,
        },
        rule::DefinitionKey,
        target::{TargetContext, TargetEntity, TargetPool},
    },
};

pub fn satisfied_conditions(
    conditions: &[ParsedCondition],
    trigger_key: DefinitionKey,
) -> Vec<ParsedCondition> {
    conditions
        .iter()
        .map(|condition| satisfied_condition(condition, trigger_key))
        .collect()
}

pub fn satisfied_card_enchants(
    conditions: &[ParsedCondition],
    card_enchants: &[i32],
) -> Vec<ParsedCondition> {
    conditions
        .iter()
        .map(|condition| {
            let mut condition = condition.clone();
            match &mut condition.kind {
                ParsedConditionKind::CurrentCardEnchant { enchant_id }
                    if !card_enchants.is_empty()
                        && (*enchant_id == 0 || card_enchants.contains(enchant_id)) =>
                {
                    condition = ParsedCondition::always();
                }
                ParsedConditionKind::Any(groups) => {
                    for group in groups {
                        *group = satisfied_card_enchants(group, card_enchants);
                    }
                }
                ParsedConditionKind::Not(inner) => {
                    let nested = ParsedCondition {
                        opcode: condition.opcode,
                        type_name: condition.type_name.clone(),
                        kind: *inner.clone(),
                        raw_args: condition.raw_args.clone(),
                    };
                    **inner = satisfied_card_enchants(std::slice::from_ref(&nested), card_enchants)
                        [0]
                    .kind
                    .clone();
                }
                _ => {}
            }
            condition
        })
        .collect()
}

pub fn satisfied_condition(
    condition: &ParsedCondition,
    trigger_key: DefinitionKey,
) -> ParsedCondition {
    let is_satisfiable_event_marker = match condition.kind {
        ParsedConditionKind::Lifecycle(_)
        | ParsedConditionKind::AllyAttacked
        | ParsedConditionKind::EntityDead => true,
        ParsedConditionKind::None(mode) => mode != super::none::NoneMode::AllyAction,
        _ => false,
    };
    if trigger_key.matches(condition.opcode, &condition.type_name)
        && (matches!(condition.timing(), super::ConditionTiming::Event(_))
            || matches!(
                super::registry::find_key(condition.opcode, &condition.type_name)
                    .map(|definition| definition.role),
                Some(super::registry::ConditionRole::Setup { .. })
            ))
        && is_satisfiable_event_marker
    {
        return ParsedCondition::always();
    }
    let mut condition = condition.clone();
    if let ParsedConditionKind::Any(groups) = &mut condition.kind {
        for group in groups {
            for condition in group {
                *condition = satisfied_condition(condition, trigger_key);
            }
        }
    }
    condition
}

pub fn conditions_match(
    conditions: &[ParsedCondition],
    source_uid: i64,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
    pool: &TargetPool,
    context: TargetContext,
) -> bool {
    conditions.iter().all(|condition| {
        condition_matches(
            condition,
            source_uid,
            condition_targets,
            managers,
            pool,
            context,
        )
    })
}

pub(crate) fn conditions_fire_count(
    conditions: &[ParsedCondition],
    source_uid: i64,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
    pool: &TargetPool,
    context: TargetContext,
) -> i32 {
    if conditions.is_empty() {
        return 1;
    }

    if !conditions_match(
        conditions,
        source_uid,
        condition_targets,
        managers,
        pool,
        context,
    ) {
        return 0;
    }

    conditions
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
}

fn condition_repeat_count(
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
        ParsedConditionKind::TeamLostHpPercent {
            team_type,
            interval_permille,
            max_count,
        } => managers.map(|managers| {
            super::hp::team_lost_hp_count(
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
                    super::ex_point::per_ex_point_count(
                        *threshold,
                        managers.ex_point.get(source_uid),
                    )
                })
                .unwrap_or_default(),
        ),
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
                    super::buff::per_type_layer_count(
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
                    super::buff::added_count_repeats(condition, source_uid, managers, context)
                })
                .unwrap_or_default(),
        ),
        ParsedConditionKind::BurnOverflow => Some(context.buff_overflow_amount),
        _ => None,
    }
}

pub(crate) fn condition_matches(
    condition: &ParsedCondition,
    source_uid: i64,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
    pool: &TargetPool,
    context: TargetContext,
) -> bool {
    condition_kind_matches(
        condition,
        &condition.kind,
        source_uid,
        condition_targets,
        managers,
        pool,
        context,
    )
}

fn condition_kind_matches(
    condition: &ParsedCondition,
    kind: &ParsedConditionKind,
    source_uid: i64,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
    pool: &TargetPool,
    context: TargetContext,
) -> bool {
    match kind {
        ParsedConditionKind::Any(groups) => groups.iter().any(|group| {
            conditions_match(
                group,
                source_uid,
                condition_targets,
                managers,
                pool,
                context,
            )
        }),
        ParsedConditionKind::Not(inner) => !condition_kind_matches(
            condition,
            inner,
            source_uid,
            condition_targets,
            managers,
            pool,
            context,
        ),
        ParsedConditionKind::Lifecycle(_) => false,
        ParsedConditionKind::RoundInterval {
            start_round,
            period,
        } => {
            *period > 0
                && context.current_round >= *start_round
                && (context.current_round - *start_round) % *period == 0
        }
        ParsedConditionKind::ActionOrder(order) => context.action_order == *order,
        ParsedConditionKind::ActionOrderRange { start, count } => {
            *count > 0
                && context.action_order >= *start
                && context.action_order < start.saturating_add(*count)
        }
        ParsedConditionKind::None(super::none::NoneMode::AllyAction) => {
            condition_targets.contains(&source_uid)
                || (context.active_skill_source_uid != 0
                    && condition_targets.contains(&context.active_skill_source_uid))
        }
        ParsedConditionKind::None(_) => condition.allows_active_skill(),
        ParsedConditionKind::BuffId { mode, buff_ids } => {
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
            }
        }
        ParsedConditionKind::BuffIdCount {
            buff_ids,
            compare,
            threshold,
        } => {
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
        ParsedConditionKind::BuffTypeCount {
            type_ids,
            compare,
            threshold,
        } => {
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
        ParsedConditionKind::BuffGroup(group_ids) => managers.is_some_and(|managers| {
            condition_targets
                .iter()
                .any(|uid| managers.buff.buff_group_type_count(*uid, group_ids) > 0)
        }),
        ParsedConditionKind::NoBuffGroup(group_ids) => managers.is_some_and(|managers| {
            condition_targets
                .iter()
                .all(|uid| managers.buff.buff_group_type_count(*uid, group_ids) == 0)
        }),
        ParsedConditionKind::FromBuffAndToBuff {
            from_buff_id,
            to_buff_id,
        } => managers.is_some_and(|managers| {
            managers
                .buff
                .has_active_buff_id_or_type(source_uid, *from_buff_id)
                && condition_targets.iter().any(|target_uid| {
                    managers
                        .buff
                        .has_active_buff_id_or_type(*target_uid, *to_buff_id)
                })
        }),
        ParsedConditionKind::EnemyHighestBuffTypeCount { type_id, threshold } => managers
            .is_some_and(|managers| {
                pool.enemies(source_uid, true)
                    .iter()
                    .map(|enemy| managers.buff.buff_type_amount(enemy.uid, *type_id))
                    .max()
                    .unwrap_or_default()
                    >= *threshold
            }),
        ParsedConditionKind::BurnOverflow => context.buff_overflow_amount > 0,
        ParsedConditionKind::PerBuffTypeLayer { type_ids, min, .. } => {
            managers.is_some_and(|managers| {
                condition_targets.iter().any(|uid| {
                    type_ids
                        .iter()
                        .map(|type_id| managers.buff.buff_type_amount(*uid, *type_id))
                        .sum::<i32>()
                        >= *min
                })
            })
        }
        ParsedConditionKind::BuffStatusCount {
            status_ids,
            compare,
            threshold,
        } => managers.is_some_and(|managers| {
            let amount = condition_targets
                .iter()
                .map(|uid| managers.buff.buff_status_count(*uid, status_ids))
                .sum();
            compare_value(amount, *compare, *threshold)
        }),
        ParsedConditionKind::BuffAdded(buff_ids) => {
            context.added_buff_amount > 0 && buff_ids.contains(&context.added_buff_id)
        }
        ParsedConditionKind::BuffRemoved(buff_ids) => {
            context.removed_buff_target_uid != 0
                && condition_targets.contains(&context.removed_buff_target_uid)
                && buff_ids.contains(&context.removed_buff_id)
        }
        ParsedConditionKind::AccBuffAddedCount { .. } => managers.is_some_and(|managers| {
            super::buff::added_count_repeats(condition, source_uid, managers, context) > 0
        }),
        ParsedConditionKind::HpPermille { compare, threshold } => {
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
        ParsedConditionKind::PerHp { interval_permille } => managers.is_some_and(|managers| {
            *interval_permille > 0
                && condition_targets
                    .iter()
                    .any(|uid| managers.hp.current(*uid) > 0)
        }),
        ParsedConditionKind::TeamLostHpPercent {
            team_type,
            interval_permille,
            max_count,
        } => managers.is_some_and(|managers| {
            super::hp::team_lost_hp_count(
                *team_type,
                *interval_permille,
                *max_count,
                managers,
                pool,
            ) > 0
        }),
        ParsedConditionKind::BloodPoolMax { min, max } => {
            (*min..=*max).contains(&context.blood_pool_max)
        }
        ParsedConditionKind::BloodPoolValue {
            min,
            max,
            config_effect,
        } => {
            let value = match config_effect {
                0 => context.blood_pool_value,
                1 => context.heat_scale_raw_value,
                _ => return false,
            };
            (*min..=*max).contains(&value)
        }
        ParsedConditionKind::CurrentCardEnchant { .. } => false,
        ParsedConditionKind::ExPoint { compare, threshold } => {
            let Some(managers) = managers else {
                return false;
            };
            condition_targets
                .iter()
                .any(|uid| compare_value(managers.ex_point.get(*uid), *compare, *threshold))
        }
        ParsedConditionKind::Synchronization { threshold } => {
            let Some(managers) = managers else {
                return false;
            };
            condition_targets.iter().any(|uid| {
                managers.ex_point.kind(*uid)
                    == crate::engine::manager::ex_point::ExPointKind::Synchronization.as_wire()
                    && managers.ex_point.get(*uid) >= *threshold
            })
        }
        ParsedConditionKind::PerExPoint { threshold } => managers.is_some_and(|managers| {
            condition_targets
                .iter()
                .any(|uid| managers.ex_point.get(*uid) >= *threshold)
        }),
        ParsedConditionKind::Random { threshold } => context
            .condition_random_roll
            .is_some_and(|roll| roll < *threshold),
        ParsedConditionKind::ExPointDecrease { threshold } => {
            context.ex_point_changed_uid != 0
                && context.ex_point_delta < 0
                && -context.ex_point_delta >= *threshold
                && condition_targets.contains(&context.ex_point_changed_uid)
        }
        ParsedConditionKind::ExPointLost => {
            context.ex_point_changed_uid != 0
                && context.ex_point_delta < 0
                && condition_targets.contains(&context.ex_point_changed_uid)
        }
        ParsedConditionKind::ExPointIncrChange { .. } => false,
        ParsedConditionKind::PowerCompare {
            compare_code,
            power_id,
            threshold,
        } => {
            let Some(managers) = managers else {
                return false;
            };
            compare_resource(
                managers.eureka.get(source_uid, *power_id).current,
                *compare_code,
                *threshold,
            )
        }
        ParsedConditionKind::ConduitExPoint {
            compare_code,
            threshold,
        } => managers.is_some_and(|managers| {
            condition_targets
                .iter()
                .any(|uid| compare_resource(managers.ex_point.get(*uid), *compare_code, *threshold))
        }),
        ParsedConditionKind::ConduitSkillGroup { group } => managers.is_some_and(|managers| {
            condition_targets
                .iter()
                .any(|uid| managers.conduit.selected_group(*uid) == Some(*group))
        }),
        ParsedConditionKind::PowerIncrChange { .. }
        | ParsedConditionKind::PerConduitCurrentCost { .. }
        | ParsedConditionKind::CurrentEntityPowerDecrease
        | ParsedConditionKind::PowerUseAddBuff { .. } => false,
        ParsedConditionKind::PowerOverflow {
            power_id,
            max_count,
        } => managers.is_some_and(|managers| {
            managers.eureka.round_change(source_uid, *power_id).overflow > 0 && *max_count > 0
        }),
        ParsedConditionKind::PowerConsumed {
            power_id,
            max_count,
        } => managers.is_some_and(|managers| {
            managers.eureka.round_change(source_uid, *power_id).consumed > 0 && *max_count > 0
        }),
        ParsedConditionKind::LostPower {
            power_id,
            threshold,
        } => context.lost_power_id == *power_id && context.lost_power_amount >= *threshold,
        ParsedConditionKind::TargetAttacked => {
            context.hit_target_uid != 0 && condition_targets.contains(&context.hit_target_uid)
        }
        ParsedConditionKind::AllyAttacked => {
            context.hit_target_uid != 0
                && context.hit_target_uid != source_uid
                && pool.entity(source_uid).is_some()
                && pool.entity(context.hit_target_uid).is_some()
                && pool.source_is_attacker(source_uid)
                    == pool.source_is_attacker(context.hit_target_uid)
        }
        ParsedConditionKind::ShareDamage => {
            context.hit_damage_from
                == Some(crate::engine::manager::hp::HurtDamageFromType::ShareHurt)
        }
        ParsedConditionKind::Assassinate => context.active_skill_assassinate,
        ParsedConditionKind::TeammateInjuryCount {
            persistent,
            threshold,
        } => {
            let count = if *persistent {
                managers
                    .map(|managers| {
                        managers.buff.buff_act_value(
                            source_uid,
                            crate::engine::skill::buff_act::registry::BuffActKind::TeammateInjuryCount,
                        )
                    })
                    .unwrap_or(context.teammate_injury_count_not_reset)
            } else {
                context.teammate_injury_count
            };
            count >= *threshold
        }
        ParsedConditionKind::TeamInjuryCountRound { max_count } => {
            *max_count > 0 && context.team_injury_count_round > 0
        }
        ParsedConditionKind::EntityDead => managers.is_some_and(|managers| {
            condition_targets
                .iter()
                .any(|uid| managers.hp.current(*uid) <= 0)
        }),
        ParsedConditionKind::TeammateDead => {
            context.runtime_target_uid != 0
                && context.runtime_target_uid != source_uid
                && pool.entity(source_uid).is_some()
                && pool.source_is_attacker(source_uid)
                    == pool.source_is_attacker(context.runtime_target_uid)
        }
        ParsedConditionKind::EnemyDead => {
            context.runtime_target_uid != 0
                && pool.entity(source_uid).is_some()
                && pool.source_is_attacker(source_uid)
                    != pool.source_is_attacker(context.runtime_target_uid)
        }
        ParsedConditionKind::SingleKillCount { threshold } => {
            context.action_kill_count >= *threshold
        }
        ParsedConditionKind::PerKillCount { divisor } => {
            *divisor > 0 && context.action_kill_count >= *divisor
        }
        ParsedConditionKind::TeamEntityExited => managers.is_some_and(|managers| {
            context.runtime_target_uid != 0
                && managers.hp.current(context.runtime_target_uid) <= 0
                && pool.source_is_attacker(source_uid)
                    != pool.source_is_attacker(context.runtime_target_uid)
        }),
        ParsedConditionKind::MultiHpSegment(segment) => context.multi_hp_segment == *segment,
        ParsedConditionKind::TargetCareer(careers) => condition_targets
            .iter()
            .filter_map(|uid| pool.entity(*uid))
            .any(|entity| careers.iter().any(|career| entity.has_career(*career))),
        ParsedConditionKind::TargetSharesCasterCareer { param } => {
            let Some(source) = pool.entity(source_uid) else {
                return false;
            };
            let shares = source.career != 0
                && condition_targets
                    .iter()
                    .filter_map(|uid| pool.entity(*uid))
                    .any(|entity| entity.shares_career_with(source));
            shares == (*param == 0)
        }
        ParsedConditionKind::PerTargetCareerCount { careers, threshold } => {
            per_target_career_count(source_uid, careers, *threshold, pool) > 0
        }
        ParsedConditionKind::TeamCareerCount {
            careers,
            compare,
            threshold,
        } => compare_value(
            team_career_count(source_uid, careers, pool),
            *compare,
            *threshold,
        ),
        ParsedConditionKind::OtherAllyDamageTypeCount {
            damage_type,
            max_count,
        } => other_ally_damage_type_count(source_uid, *damage_type, *max_count, pool) > 0,
        ParsedConditionKind::ActiveSkillId(skill_ids) => {
            context.active_skill_id != 0 && skill_ids.contains(&context.active_skill_id)
        }
        ParsedConditionKind::CanUseSkill(skill_ids) => {
            // A SkillAction is published only after the cast is accepted. At that
            // boundary the committed active skill is the server-side proof that
            // this configured skill was usable.
            context.active_skill_id != 0 && skill_ids.contains(&context.active_skill_id)
        }
        ParsedConditionKind::ActiveUseSkill { slot } => {
            context.direct_skill_body
                && context.extra_skill_kind == 0
                && active_skill_has_real_source(pool, context)
                && (*slot == 0 || context.active_skill_slot == *slot)
        }
        ParsedConditionKind::UseSkillRank(ranks) => {
            context.active_skill_source_uid == source_uid
                && context.active_skill_rank != 0
                && ranks.contains(&context.active_skill_rank)
        }
        ParsedConditionKind::UseHurtSkill => {
            context.active_skill_is_attack && active_skill_has_real_source(pool, context)
        }
        ParsedConditionKind::SpecificSkill { group, rank } => {
            pool.entity(source_uid).is_some_and(|source| {
                let matches_group = match group {
                    0 => {
                        source.skill_group1.contains(&context.active_skill_id)
                            || source.skill_group2.contains(&context.active_skill_id)
                    }
                    1 => source.skill_group1.contains(&context.active_skill_id),
                    2 => source.skill_group2.contains(&context.active_skill_id),
                    3 => source.ex_skill == context.active_skill_id,
                    4 => {
                        source.skill_group1.contains(&context.active_skill_id)
                            || source.skill_group2.contains(&context.active_skill_id)
                    }
                    5 => {
                        source.skill_group1.contains(&context.active_skill_id)
                            || source.skill_group2.contains(&context.active_skill_id)
                    }
                    _ => false,
                };
                matches_group && (*rank <= 0 || context.active_skill_rank == *rank)
            })
        }
        ParsedConditionKind::UseExSkill => pool.entity(source_uid).is_some_and(|source| {
            source.ex_skill != 0
                && crate::engine::mechanic::card::CardMechanic
                    .is_ultimate_skill(context.active_skill_id, source)
        }),
        ParsedConditionKind::TargetUseExSkill => {
            context.active_skill_source_uid != 0
                && condition_targets.contains(&context.active_skill_source_uid)
                && pool
                    .entity(context.active_skill_source_uid)
                    .is_some_and(|actor| {
                        actor.ex_skill != 0
                            && crate::engine::mechanic::card::CardMechanic
                                .is_ultimate_skill(context.active_skill_id, actor)
                    })
        }
        ParsedConditionKind::TeammateUseExSkill => pool.allies(source_uid).iter().any(|ally| {
            ally.uid != source_uid
                && ally.uid == context.active_skill_source_uid
                && ally.ex_skill != 0
                && crate::engine::mechanic::card::CardMechanic
                    .is_ultimate_skill(context.active_skill_id, ally)
        }),
        ParsedConditionKind::ActiveSkillRank { compare, ranks } => {
            context.active_skill_rank != 0
                && ranks
                    .iter()
                    .any(|rank| compare_value(context.active_skill_rank, *compare, *rank))
        }
        ParsedConditionKind::ActiveSkillType(skill_type) => {
            context.active_skill_type != 0 && context.active_skill_type == *skill_type
        }
        ParsedConditionKind::ActiveSkillEffectTag(tags) => {
            context.active_skill_effect_tag != 0 && tags.contains(&context.active_skill_effect_tag)
        }
        ParsedConditionKind::DamageTargetCountKind(kind) => {
            context.damage_target_count_kind == *kind
        }
        ParsedConditionKind::AttackerDamageType(damage_type) => pool
            .entity(context.hit_source_uid)
            .is_some_and(|attacker| attacker.damage_type == *damage_type),
        ParsedConditionKind::AttackCrit => context.action_crit_count > 0,
        ParsedConditionKind::BeforeCrit => context.action_crit_count > 0,
        ParsedConditionKind::HurtRestrained | ParsedConditionKind::HurtNotRestrained => {
            let Some(attacker) = pool.entity(context.hit_source_uid) else {
                return false;
            };
            let Some(defender) = pool.entity(context.hit_target_uid) else {
                return false;
            };
            let forces_restraint = managers.is_some_and(|managers| {
                managers
                    .buff
                    .active_features(&managers.hp)
                    .iter()
                    .filter(|feature| feature.owner_uid == attacker.uid)
                    .any(crate::engine::skill::buff_act::forces_career_restraint)
            });
            let restrained = forces_restraint
                || crate::engine::damage::handler::restrains(attacker.career, defender.career);
            restrained == matches!(condition.kind, ParsedConditionKind::HurtRestrained)
        }
        ParsedConditionKind::EntityCount {
            scope,
            compare,
            count,
        } => entity_count_matches(*scope, *compare, *count, source_uid, managers, pool),
        ParsedConditionKind::SummonedCount {
            summoned_id,
            required_level,
            compare,
            count,
        } => managers.is_some_and(|managers| {
            compare_value(
                managers
                    .summon
                    .count(source_uid, *summoned_id, *required_level),
                *compare,
                *count,
            )
        }),
        ParsedConditionKind::GroupSummonedCount {
            owner_model_id,
            required_level,
            compare,
            count,
        } => managers.is_some_and(|managers| {
            let amount = pool
                .allies(source_uid)
                .iter()
                .filter(|entity| entity.model_id == *owner_model_id)
                .map(|entity| managers.summon.total(entity.uid, *required_level))
                .sum();
            compare_value(amount, *compare, *count)
        }),
        ParsedConditionKind::BattleTagCount {
            tag_id,
            compare,
            threshold,
        } => compare_value(
            pool.allies(source_uid)
                .iter()
                .filter(|entity| entity.battle_tags.contains(tag_id))
                .count() as i32,
            *compare,
            *threshold,
        ),
        ParsedConditionKind::TargetIdentity { mode, value } => {
            target_identity_matches(*mode, *value, source_uid, condition_targets, pool, context)
        }
        ParsedConditionKind::TeamContainsModels(model_ids) => {
            pool.allies(source_uid).iter().any(|target| {
                model_ids.contains(&target.model_id) || model_ids.contains(&(target.model_id / 10))
            })
        }
        ParsedConditionKind::TeamModelPresence { model_ids, present } => {
            let found = pool.allies(source_uid).iter().any(|target| {
                model_ids.contains(&target.model_id) || model_ids.contains(&(target.model_id / 10))
            });
            found == *present
        }
        ParsedConditionKind::ExtraAction { mode, kinds } => {
            extra_action_kind_matches(context.extra_skill_kind, kinds)
                && match mode {
                    super::extra::ExtraActionConditionMode::OtherAllyAction => {
                        context.active_skill_source_uid != 0
                            && condition_targets.contains(&context.active_skill_source_uid)
                    }
                    _ => true,
                }
        }
        ParsedConditionKind::InMagicCircleId(ids) => {
            let field_id = current_magic_circle_id(source_uid, managers, pool, context);
            field_id != 0 && ids.contains(&field_id)
        }
        ParsedConditionKind::NotInMagicCircleId(ids) => {
            let field_id = current_magic_circle_id(source_uid, managers, pool, context);
            field_id == 0 || !ids.contains(&field_id)
        }
        ParsedConditionKind::AddedMagicCircle(ids) => {
            context.added_magic_circle_id != 0
                && (ids.contains(&0) || ids.contains(&context.added_magic_circle_id))
        }
        ParsedConditionKind::RemovedMagicCircle(ids) => {
            context.removed_magic_circle_id != 0
                && (ids.contains(&0) || ids.contains(&context.removed_magic_circle_id))
        }
        ParsedConditionKind::BuffFeatureTriggered { act_id } => {
            context.triggered_buff_act_id == *act_id
        }
        ParsedConditionKind::MasterHalo => condition_targets
            .iter()
            .any(|uid| has_master_halo(*uid, managers)),
        ParsedConditionKind::NoActionRound => !context.owner_played_card,
        ParsedConditionKind::Unsupported(_) => false,
    }
}

fn current_magic_circle_id(
    source_uid: i64,
    managers: Option<&BattleManagers>,
    pool: &TargetPool,
    context: TargetContext,
) -> i32 {
    managers
        .and_then(|managers| {
            let team = if pool.source_is_attacker(source_uid) {
                1
            } else {
                2
            };
            managers.field.get(team)
        })
        .map(|field| field.definition.field_id)
        .unwrap_or(context.magic_circle_id)
}

fn active_skill_has_real_source(pool: &TargetPool, context: TargetContext) -> bool {
    context.active_skill_source_uid == 0
        || pool
            .entities()
            .any(|entity| entity.uid == context.active_skill_source_uid)
}

fn has_master_halo(uid: i64, managers: Option<&BattleManagers>) -> bool {
    let is_halo = |raw: &str| {
        raw.split('|').any(|feature| {
            matches!(
                feature
                    .split('#')
                    .next()
                    .and_then(|value| value.parse().ok()),
                Some(771 | 772 | 822)
            )
        })
    };
    managers.is_some_and(|managers| {
        managers
            .buff
            .active_for(uid)
            .filter_map(|buff| {
                config::try_get()?
                    .skill_buff
                    .get(buff.buff_id?)
                    .map(|row| &row.features)
            })
            .any(|features| is_halo(features))
    })
}

fn extra_action_kind_matches(actual: i32, expected: &[i32]) -> bool {
    expected.contains(&actual) || (matches!(actual, 2 | 3) && expected.contains(&1))
}

fn compare_value(value: i32, compare: ConditionCompare, expected: i32) -> bool {
    match compare {
        ConditionCompare::Equal => value == expected,
        ConditionCompare::NotEqual => value != expected,
        ConditionCompare::GreaterThan => value > expected,
        ConditionCompare::GreaterThanOrEqual => value >= expected,
        ConditionCompare::LessThan => value < expected,
        ConditionCompare::LessThanOrEqual => value <= expected,
    }
}

pub(crate) fn compare_resource(value: i32, compare_code: i32, expected: i32) -> bool {
    match compare_code {
        1 => value >= expected,
        2 => value <= expected,
        3 => value == expected,
        4 => value != expected,
        6 => value <= expected,
        7 => value < expected,
        _ => false,
    }
}

fn entity_count_matches(
    scope: EntityCountScope,
    compare: ConditionCompare,
    expected: i32,
    source_uid: i64,
    managers: Option<&BattleManagers>,
    pool: &TargetPool,
) -> bool {
    if pool.entity(source_uid).is_none() {
        return false;
    }

    let source_is_attacker = pool.source_is_attacker(source_uid);
    let alive_count = |entities: &[TargetEntity]| {
        entities
            .iter()
            .filter(|entity| managers.is_none_or(|managers| managers.hp.current(entity.uid) > 0))
            .count()
    };
    let count = match scope {
        EntityCountScope::EnemyTargets
        | EntityCountScope::AliveEnemies
        | EntityCountScope::AliveEnemiesIncludeSp => alive_count(pool.enemies(source_uid, false)),
        EntityCountScope::AliveTeammates => alive_count(pool.allies(source_uid)),
        EntityCountScope::AliveTeammatesNoSp => {
            if source_is_attacker {
                alive_count(&pool.attacker_main)
            } else {
                alive_count(&pool.defender_main)
            }
        }
        EntityCountScope::TeamSize | EntityCountScope::HeroCount => {
            if source_is_attacker {
                pool.attacker_main.len()
            } else {
                pool.defender_main.len()
            }
        }
    } as i32;

    compare_value(count, compare, expected)
}

fn per_target_career_count(
    source_uid: i64,
    careers: &[i32],
    threshold: i32,
    pool: &TargetPool,
) -> i32 {
    let team = if pool.source_is_attacker(source_uid) {
        &pool.attacker_main
    } else {
        &pool.defender_main
    };
    let count = team
        .iter()
        .filter(|entity| {
            entity.uid != source_uid && careers.iter().any(|career| entity.has_career(*career))
        })
        .count() as i32;
    if threshold > 0 {
        count.min(threshold)
    } else {
        count
    }
}

fn team_career_count(source_uid: i64, careers: &[i32], pool: &TargetPool) -> i32 {
    let team = if pool.source_is_attacker(source_uid) {
        &pool.attacker_main
    } else {
        &pool.defender_main
    };
    team.iter()
        .filter(|entity| careers.iter().any(|career| entity.has_career(*career)))
        .count() as i32
}

fn other_ally_damage_type_count(
    source_uid: i64,
    damage_type: crate::engine::skill::target::EntityDamageType,
    max_count: i32,
    pool: &TargetPool,
) -> i32 {
    pool.allies(source_uid)
        .iter()
        .filter(|entity| entity.uid != source_uid && entity.damage_type == damage_type)
        .count()
        .min(max_count.max(0) as usize) as i32
}

fn target_identity_matches(
    mode: TargetIdentityMode,
    value: i32,
    source_uid: i64,
    condition_targets: &[i64],
    pool: &TargetPool,
    context: TargetContext,
) -> bool {
    let targets = identity_targets(condition_targets, pool, context);
    match mode {
        TargetIdentityMode::TargetIsSelf => targets.iter().any(|target| target.uid == source_uid),
        TargetIdentityMode::TargetIsAllyNotSelf => targets.iter().any(|target| {
            target.uid != source_uid
                && pool.source_is_attacker(target.uid) == pool.source_is_attacker(source_uid)
        }),
        TargetIdentityMode::TargetModelId => targets
            .iter()
            .any(|target| target.model_id == value || target.model_id / 10 == value),
        TargetIdentityMode::TargetPosition => targets.iter().any(|target| target.position == value),
    }
}

fn identity_targets<'a>(
    condition_targets: &[i64],
    pool: &'a TargetPool,
    context: TargetContext,
) -> Vec<&'a TargetEntity> {
    if context.runtime_target_uid != 0
        && let Some(target) = pool.entity(context.runtime_target_uid)
    {
        return vec![target];
    }
    let targets: Vec<_> = condition_targets
        .iter()
        .filter_map(|uid| pool.entity(*uid))
        .collect();
    targets
}

#[cfg(test)]
mod test;
