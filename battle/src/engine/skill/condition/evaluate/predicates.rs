use super::*;

pub(super) fn condition_kind_matches(
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
        ParsedConditionKind::RankAtLeast(minimum) => managers.is_some_and(|managers| {
            super::super::rank::matches(*minimum, condition_targets, managers, pool)
        }),
        ParsedConditionKind::SovereignCardCreated => {
            context.runtime_target_uid != 0 && context.runtime_target_uid == source_uid
        }
        ParsedConditionKind::RoundInterval {
            start_round,
            period,
        } => {
            super::super::lifecycle::interval_matches(context.current_round, *start_round, *period)
        }
        ParsedConditionKind::RoundStartInterval {
            start_round,
            period,
        } => super::super::lifecycle::interval_matches(
            context
                .defender_start_round
                .unwrap_or(context.current_round),
            *start_round,
            *period,
        ),
        ParsedConditionKind::ActionOrder(order) => context.action_order == *order,
        ParsedConditionKind::ActionOrderRange { start, count } => {
            *count > 0
                && context.action_order >= *start
                && context.action_order < start.saturating_add(*count)
        }
        ParsedConditionKind::None(super::super::none::NoneMode::AllyAction) => {
            condition_targets.contains(&source_uid)
                || (context.active_skill_source_uid != 0
                    && condition_targets.contains(&context.active_skill_source_uid))
        }
        ParsedConditionKind::None(super::super::none::NoneMode::EmitterSkillAction) => {
            context.active_skill_source_uid == crate::engine::manager::emitter::UID
        }
        ParsedConditionKind::None(_) => condition.allows_active_skill(),
        ParsedConditionKind::BuffId { mode, buff_ids } => {
            buff::buff_id(mode, buff_ids, condition_targets, managers)
        }
        ParsedConditionKind::BuffIdCount {
            buff_ids,
            compare,
            threshold,
        } => buff::buff_id_count(buff_ids, compare, threshold, condition_targets, managers),
        ParsedConditionKind::BuffIdThreshold {
            buff_ids,
            threshold,
        } => buff::buff_id_threshold(buff_ids, threshold, condition_targets, managers),
        ParsedConditionKind::PlayerBuffPresence { present, buff_id } => {
            buff::player_buff_presence(present, buff_id, managers)
        }
        ParsedConditionKind::BuffTypeCount {
            type_ids,
            compare,
            threshold,
        } => buff::buff_type_count(type_ids, compare, threshold, condition_targets, managers),
        ParsedConditionKind::AnyTargetBuffTypeCount {
            type_ids,
            threshold,
        } => buff::any_target_buff_type_count(type_ids, threshold, condition_targets, managers),
        ParsedConditionKind::AllTargetBuffTypeCount {
            type_ids,
            threshold,
        } => buff::all_target_buff_type_count(type_ids, threshold, condition_targets, managers),
        ParsedConditionKind::BuffGroup(group_ids) => managers.is_some_and(|managers| {
            condition_targets
                .iter()
                .any(|uid| managers.buff.buff_group_type_count(*uid, group_ids) > 0)
        }),
        ParsedConditionKind::PerBuffGroupCount { group_id } => managers.is_some_and(|managers| {
            condition_targets
                .iter()
                .any(|uid| managers.buff.buff_group_amount(*uid, *group_id) > 0)
        }),
        ParsedConditionKind::PerBullet { divisor, .. } => managers.is_some_and(|managers| {
            managers.buff.buff_act_type_count(
                condition_targets,
                crate::engine::skill::buff_act::registry::BuffActKind::Bullet,
            ) >= *divisor
        }),
        ParsedConditionKind::NoBuffGroup(group_ids) => managers.is_some_and(|managers| {
            condition_targets
                .iter()
                .all(|uid| managers.buff.buff_group_type_count(*uid, group_ids) == 0)
        }),
        ParsedConditionKind::FromBuffAndToBuff {
            from_buff_id,
            to_buff_id,
        } => buff::from_buff_and_to_buff(
            from_buff_id,
            to_buff_id,
            source_uid,
            condition_targets,
            managers,
        ),
        ParsedConditionKind::SelfBuffTypeTargetBuffTypes {
            self_type_id,
            target_type_ids,
        } => buff::self_buff_type_target_buff_types(
            self_type_id,
            target_type_ids,
            source_uid,
            condition_targets,
            managers,
            context,
        ),
        ParsedConditionKind::EnemyHighestBuffTypeCount { type_id, threshold } => {
            buff::enemy_highest_buff_type_count(type_id, threshold, source_uid, managers, pool)
        }
        ParsedConditionKind::BurnOverflow => context.buff_overflow_amount > 0,
        ParsedConditionKind::PerBuffTypeLayer { type_ids, min, .. } => {
            buff::per_buff_type_layer(type_ids, min, condition_targets, managers)
        }
        ParsedConditionKind::BuffStatusCount {
            status_ids,
            compare,
            threshold,
        } => buff::buff_status_count(status_ids, compare, threshold, condition_targets, managers),
        ParsedConditionKind::PrimaryStatusAbsent(statuses) => {
            buff::primary_status_absent(statuses, condition_targets, managers)
        }
        ParsedConditionKind::VisibleBuffStatusTypeCount {
            status_ids,
            compare,
            threshold,
        } => buff::visible_buff_status_type_count(
            status_ids,
            compare,
            threshold,
            condition_targets,
            managers,
        ),
        ParsedConditionKind::VisibleStackedStatusTypeCount {
            status_ids,
            compare,
            threshold,
        } => buff::visible_stacked_status_type_count(
            status_ids,
            compare,
            threshold,
            condition_targets,
            managers,
        ),
        ParsedConditionKind::PerTeamBuffStatusTypeCount {
            status_ids,
            divisor,
            ..
        } => {
            buff::per_team_buff_status_type_count(status_ids, divisor, condition_targets, managers)
        }
        ParsedConditionKind::PerConduitCounter {
            kind,
            divisor,
            max_count,
        } => managers.is_some_and(|managers| {
            conduit_counter_count(source_uid, *kind, *divisor, *max_count, managers, pool) > 0
        }),
        // "BuffIdAdd" also lists buff types: Master Assassin's ally grant fires on type 229502.
        ParsedConditionKind::BuffAdded(buff_ids) => {
            buff::buff_added(buff_ids, condition_targets, context)
        }
        ParsedConditionKind::BuffTypeAdded(type_ids) => {
            buff::buff_type_added(type_ids, condition_targets, context)
        }
        ParsedConditionKind::BuffRemoved(buff_ids) => {
            buff::buff_removed(buff_ids, condition_targets, context)
        }
        ParsedConditionKind::BuffExpired(buff_ids) => {
            buff::buff_expired(buff_ids, condition_targets, context)
        }
        ParsedConditionKind::AccBuffAddedCount { .. } => managers.is_some_and(|managers| {
            super::super::buff::added_count_repeats(condition, source_uid, managers, context) > 0
        }),
        ParsedConditionKind::AccBuffTypeAddedCount { .. } => managers.is_some_and(|managers| {
            super::super::buff::added_type_count_repeats(condition, managers, context) > 0
        }),
        ParsedConditionKind::HpPermille { compare, threshold } => {
            resources::hp_permille(compare, threshold, condition_targets, managers)
        }
        ParsedConditionKind::PerHp { interval_permille } => {
            resources::per_hp(interval_permille, condition_targets, managers)
        }
        ParsedConditionKind::PerLostHp { interval_permille } => {
            resources::per_lost_hp(interval_permille, condition_targets, managers, context)
        }
        ParsedConditionKind::TeamLostHpPercent {
            team_type,
            interval_permille,
            max_count,
        } => {
            resources::team_lost_hp_percent(team_type, interval_permille, max_count, managers, pool)
        }
        ParsedConditionKind::BloodPoolMax { min, max } => {
            (*min..=*max).contains(&context.blood_pool_max)
        }
        ParsedConditionKind::BloodPoolValue {
            min,
            max,
            config_effect,
        } => resources::blood_pool_value(min, max, config_effect, context),
        ParsedConditionKind::CurrentCardEnchant { .. } => false,
        ParsedConditionKind::CardAnchor(order) => managers.is_some_and(|managers| {
            card_anchor_matches(source_uid, context.active_card_index, *order, managers)
        }),
        ParsedConditionKind::HandSkillPresence(skill_ids) => managers.is_some_and(|managers| {
            managers.card.hand().iter().any(|card| {
                card.skill_id
                    .is_some_and(|skill_id| skill_ids.contains(&skill_id))
            })
        }),
        ParsedConditionKind::RoundUsedMinimumRank {
            minimum_rank,
            threshold,
        } => actions::round_used_minimum_rank(minimum_rank, threshold, source_uid, managers, pool),
        ParsedConditionKind::ExPoint { compare, threshold } => {
            resources::ex_point(compare, threshold, condition_targets, managers)
        }
        ParsedConditionKind::ExPointFull => resources::ex_point_full(condition_targets, managers),
        ParsedConditionKind::ExSkillLevel(level) => condition_targets.iter().any(|uid| {
            pool.entity(*uid)
                .is_some_and(|entity| entity.ex_skill_level == *level)
        }),
        ParsedConditionKind::ExSkillLevels(levels) => condition_targets.iter().any(|uid| {
            pool.entity(*uid)
                .is_some_and(|entity| levels.contains(&entity.ex_skill_level))
        }),
        ParsedConditionKind::Synchronization { threshold } => {
            resources::synchronization(threshold, condition_targets, managers)
        }
        ParsedConditionKind::PerExPoint { threshold } => {
            resources::per_ex_point(threshold, source_uid, condition_targets, managers, context)
        }
        ParsedConditionKind::PerEureka => resources::per_eureka(source_uid, managers),
        ParsedConditionKind::Random { threshold } => context
            .condition_random_roll
            .is_some_and(|roll| roll < *threshold),
        ParsedConditionKind::Rouge2AttributeRandom { multiplier } => {
            managers.is_some_and(|managers| {
                context.condition_random_roll.is_some_and(|roll| {
                    // attr999 (rouge2_relics_attr type 3): the leader career's recommended attributes
                    let career = managers.buff.mode_career();
                    let attributes = managers
                        .catalog()
                        .rouge2_recommended_attributes(career)
                        .into_iter()
                        .map(|id| managers.buff.mode_attribute(id))
                        .fold(0i32, i32::saturating_add);
                    let threshold = attributes.saturating_mul(*multiplier).clamp(0, 1000);
                    if crate::engine::diagnostics::enabled(
                        crate::engine::diagnostics::TraceArea::Condition,
                    ) {
                        eprintln!(
                            "rouge2 chance career={career} attributes={attributes} multiplier={multiplier} threshold={threshold} roll={roll}"
                        );
                    }
                    roll < threshold
                })
            })
        }
        ParsedConditionKind::ExPointDecrease { threshold } => {
            resources::ex_point_decrease(threshold, condition_targets, context)
        }
        ParsedConditionKind::ExPointLost => resources::ex_point_lost(condition_targets, context),
        ParsedConditionKind::ExPointIncrChange { .. } => false,
        ParsedConditionKind::PowerCompare {
            compare_code,
            power_id,
            threshold,
        } => resources::power_compare(compare_code, power_id, threshold, source_uid, managers),
        ParsedConditionKind::PowerRatio {
            power_id,
            compare_code,
            threshold_permille,
        } => resources::power_ratio(
            power_id,
            compare_code,
            threshold_permille,
            source_uid,
            managers,
        ),
        ParsedConditionKind::FightProgressValue {
            progress_id,
            compare_code,
            threshold,
        } => managers
            .and_then(|managers| managers.fight_progress.get(*progress_id))
            .is_some_and(|progress| compare_resource(progress.value, *compare_code, *threshold)),
        ParsedConditionKind::PlayerLevel { compare, threshold } => {
            managers.is_some_and(|managers| {
                managers.player.level() > 0
                    && compare_value(managers.player.level(), *compare, *threshold)
            })
        }
        ParsedConditionKind::IndicatorAtLeast {
            indicator_id,
            threshold,
        } => managers.is_some_and(|managers| managers.indicator.value(*indicator_id) >= *threshold),
        ParsedConditionKind::ConduitExPoint {
            compare_code,
            threshold,
        } => resources::conduit_ex_point(compare_code, threshold, condition_targets, managers),
        ParsedConditionKind::ConduitSkillGroup { group } => {
            resources::conduit_skill_group(group, condition_targets, managers)
        }
        ParsedConditionKind::PowerIncrChange { .. }
        | ParsedConditionKind::PerConduitCurrentCost { .. }
        | ParsedConditionKind::CurrentEntityPowerDecrease
        | ParsedConditionKind::PowerUseAddBuff { .. } => false,
        ParsedConditionKind::PowerOverflow {
            power_id,
            max_count,
        } => resources::power_overflow(power_id, max_count, source_uid, managers),
        ParsedConditionKind::PowerConsumed {
            power_id,
            max_count,
        } => resources::power_consumed(power_id, max_count, source_uid, managers),
        ParsedConditionKind::LostPower {
            power_id,
            threshold,
        } => resources::lost_power(power_id, threshold, context),
        ParsedConditionKind::TargetAttacked => {
            context.hit_target_uid != 0 && condition_targets.contains(&context.hit_target_uid)
        }
        ParsedConditionKind::HitDamagePercentMoreThan { threshold_permille } => {
            targets::hit_damage_percent_more_than(
                threshold_permille,
                condition_targets,
                managers,
                context,
            )
        }
        ParsedConditionKind::AllyAttacked => {
            context.hit_target_uid != 0
                && pool.entity(source_uid).is_some()
                && pool.entity(context.hit_target_uid).is_some()
                && pool.source_is_attacker(source_uid)
                    == pool.source_is_attacker(context.hit_target_uid)
        }
        ParsedConditionKind::ShareDamage => {
            context.hit_damage_from == Some(crate::engine::manager::hp::HurtDamageFromType::Skill)
                && context.damage_shared_uid != 0
                && condition_targets.contains(&context.damage_shared_uid)
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
        ParsedConditionKind::RejectedBuffIdOrType(id) => {
            context.rejected_buff_id == *id || context.rejected_buff_type_id == *id
        }
        ParsedConditionKind::TeamEntityExited { .. } => managers.is_some_and(|managers| {
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
            targets::target_shares_caster_career(param, source_uid, condition_targets, pool)
        }
        ParsedConditionKind::PerTargetCareerCount { careers, threshold } => {
            per_target_career_count(source_uid, careers, *threshold, pool) > 0
        }
        ParsedConditionKind::PerTeamCareerCount {
            career,
            divisor,
            opposing_team,
        } => per_team_career_count(source_uid, *career, *opposing_team, pool) >= (*divisor).max(1),
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
        ParsedConditionKind::CanUseSkill(skill_ids) => actions::can_use_skill(skill_ids, context),
        ParsedConditionKind::ActiveUseSkill { slot } => {
            actions::active_use_skill(slot, pool, context)
        }
        ParsedConditionKind::ActiveAllyUseSkill { slot } => {
            actions::active_ally_use_skill(slot, pool, context)
        }
        ParsedConditionKind::ActiveUseSkillBySource { slot } => {
            actions::active_use_skill_by_source(slot, condition_targets, pool, context)
        }
        ParsedConditionKind::UseSkillRank(ranks) => {
            actions::use_skill_rank(ranks, source_uid, context)
        }
        ParsedConditionKind::UseHurtSkill => {
            context.active_skill_is_attack && active_skill_has_real_source(pool, context)
        }
        ParsedConditionKind::UseDeviceSkill => actions::use_device_skill(pool, context),
        ParsedConditionKind::SpecificSkill { group, rank } => {
            actions::specific_skill(group, rank, source_uid, pool, context)
        }
        ParsedConditionKind::ReceivedSpecificSkill { group, rank } => {
            actions::received_specific_skill(group, rank, pool, context)
        }
        ParsedConditionKind::UseExSkill { slot } => {
            actions::use_ex_skill(slot, source_uid, managers, pool, context)
        }
        ParsedConditionKind::AfterUltimate => {
            actions::after_ultimate(source_uid, managers, pool, context)
        }
        ParsedConditionKind::TeammateUseExSkill => {
            actions::teammate_use_ex_skill(source_uid, managers, pool, context)
        }
        ParsedConditionKind::ActiveSkillSourceIsTarget => {
            context.active_skill_source_uid != 0
                && condition_targets.contains(&context.active_skill_source_uid)
        }
        ParsedConditionKind::ActiveSkillRank { compare, ranks } => {
            actions::active_skill_rank(compare, ranks, context)
        }
        ParsedConditionKind::ActiveSkillType(skill_type) => {
            context.active_skill_type != 0 && context.active_skill_type == *skill_type
        }
        ParsedConditionKind::ActiveSkillEffectTag(tags) => {
            context.active_skill_effect_tag != 0 && tags.contains(&context.active_skill_effect_tag)
        }
        ParsedConditionKind::DamageTargetCountKind(kind) => {
            context.active_skill_is_attack && context.damage_target_count_kind == *kind
        }
        ParsedConditionKind::ReceivedDamageTargetCountKind(kind) => {
            context.damage_target_count_kind == *kind
                && context.hit_target_uid != 0
                && condition_targets.contains(&context.hit_target_uid)
        }
        ParsedConditionKind::SourceDamageType(damage_type) => pool
            .entity(source_uid)
            .is_some_and(|source| source.damage_type == *damage_type),
        ParsedConditionKind::AttackerDamageType(damage_type) => pool
            .entity(context.hit_source_uid)
            .is_some_and(|attacker| attacker.damage_type == *damage_type),
        ParsedConditionKind::AttackCrit => context.action_crit_count > 0,
        ParsedConditionKind::BeforeCrit => context.action_crit_count > 0,
        ParsedConditionKind::TargetGuardBroken => context.action_guard_break_count > 0,
        ParsedConditionKind::GuardBroken => {
            !context.toughness_break_defeated
                && context.toughness_broken_uid != 0
                && condition_targets.contains(&context.toughness_broken_uid)
        }
        ParsedConditionKind::EntityBroken => managers.is_some_and(|managers| {
            condition_targets
                .iter()
                .any(|uid| managers.toughness.is_broken(*uid))
        }),
        ParsedConditionKind::HurtRestrained | ParsedConditionKind::HurtNotRestrained => {
            let (attacker_uid, defender_uid) =
                if context.hit_source_uid != 0 && context.hit_target_uid != 0 {
                    (context.hit_source_uid, context.hit_target_uid)
                } else if context.active_skill_is_attack && context.active_skill_source_uid != 0 {
                    (context.active_skill_source_uid, source_uid)
                } else {
                    return false;
                };
            if super::super::registry::find_key(condition.opcode, &condition.type_name).is_some_and(
                |definition| {
                    definition
                        .dependencies
                        .contains(&crate::engine::event::kind::EventKind::TargetAttacked)
                },
            ) && !condition_targets.contains(&defender_uid)
            {
                return false;
            }
            let Some(attacker) = pool.entity(attacker_uid) else {
                return false;
            };
            let Some(defender) = pool.entity(defender_uid) else {
                return false;
            };
            let restrained = context.hit_career_restraint.unwrap_or_else(|| {
                let forces_restraint = managers.is_some_and(|managers| {
                    managers
                        .buff
                        .active_features(&managers.hp)
                        .iter()
                        .filter(|feature| feature.owner_uid == attacker.uid)
                        .any(crate::engine::skill::buff_act::forces_career_restraint)
                });
                forces_restraint
                    || crate::engine::damage::handler::restrains_target(
                        pool.catalog(),
                        attacker.career,
                        defender,
                    )
            });
            restrained == matches!(condition.kind, ParsedConditionKind::HurtRestrained)
        }
        ParsedConditionKind::EntityCount {
            scope,
            compare,
            count,
        } => entity_count_matches(*scope, *compare, *count, source_uid, managers, pool),
        ParsedConditionKind::AliveAlliedModelCount { model_id, minimum } => {
            targets::alive_allied_model_count(model_id, minimum, source_uid, pool)
        }
        ParsedConditionKind::SummonedCount {
            summoned_id,
            required_level,
            compare,
            count,
        } => targets::summoned_count(
            summoned_id,
            required_level,
            compare,
            count,
            source_uid,
            managers,
        ),
        ParsedConditionKind::GroupSummonedCount {
            owner_model_id,
            required_level,
            compare,
            count,
        } => targets::group_summoned_count(
            owner_model_id,
            required_level,
            compare,
            count,
            source_uid,
            managers,
            pool,
        ),
        ParsedConditionKind::BattleTagCount {
            tag_id,
            compare,
            threshold,
        } => targets::battle_tag_count(tag_id, compare, threshold, source_uid, pool),
        ParsedConditionKind::TargetBattleTag(tag_id) => {
            targets::target_battle_tag(tag_id, condition_targets, pool)
        }
        ParsedConditionKind::TargetIdentity { mode, value } => {
            targets::target_identity(mode, value, source_uid, condition_targets, pool, context)
        }
        ParsedConditionKind::TeamContainsModels(model_ids) => {
            targets::team_contains_models(model_ids, source_uid, pool)
        }
        ParsedConditionKind::TeamModelPresence { model_ids, present } => {
            targets::team_model_presence(model_ids, present, source_uid, pool)
        }
        ParsedConditionKind::ExtraAction { mode, kinds } => {
            extra_action_kind_matches(context.extra_skill_kind, kinds)
                && match mode {
                    super::super::extra::ExtraActionConditionMode::OtherAllyAction => {
                        context.active_skill_source_uid != 0
                            && context.active_skill_source_uid != source_uid
                            && condition_targets.contains(&context.active_skill_source_uid)
                    }
                    super::super::extra::ExtraActionConditionMode::TargetedActor => {
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
        ParsedConditionKind::NoActionRound => {
            managers.is_some_and(|managers| !managers.card.acted_this_round(source_uid))
        }
        ParsedConditionKind::Unsupported(_) => false,
    }
}
