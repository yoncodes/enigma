use super::*;

/// Allocates configured card energy before the owning action phase begins.
pub fn run_card_energy_allocation(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    team: i32,
) -> Result<DrainResult, DrainError> {
    let ops = impromptu::allocate_team_energy_rule_ops(managers, catalog, determinism, team)
        .map_err(DrainError::Impromptu)?
        .unwrap_or_default();
    drain::run_command_group(managers, pool, catalog, determinism, context, ops)
}

pub fn run_action_queue_committed(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    team: i32,
    emitter_uid: i64,
) -> Result<DrainResult, DrainError> {
    drain::run(
        managers,
        pool,
        catalog,
        determinism,
        context,
        [RuleOp::Command(BattleCommand::Card(
            CardCommand::CommitActionQueue { team, emitter_uid },
        ))],
    )
}

pub fn run_action_phase_start(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    team: i32,
) -> Result<DrainResult, DrainError> {
    let ops = managers
        .conduit
        .action_phase_start_commands(team)
        .into_iter()
        .map(|command| RuleOp::Command(BattleCommand::Conduit(command)));
    drain::run_command_group(managers, pool, catalog, determinism, context, ops)
}

#[allow(clippy::too_many_arguments)]
pub fn run_conduit_phase(
    fight: &sonettobuf::Fight,
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &mut SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    operations: &[sonettobuf::FightDeviceOper],
) -> Result<DrainResult, DrainError> {
    let mut result = DrainResult::default();
    for operation in operations {
        let (Some(source_uid), Some(group)) = (operation.uid, operation.index) else {
            continue;
        };
        let skills = managers
            .conduit
            .selected_skills(source_uid)
            .map_err(|error| DrainError::Command(error.into()))?;
        catalog.extend_roots_and_warn(
            config::configs::get(),
            skills.iter().map(|skill| skill.skill_id),
            std::iter::empty(),
        );
        let mut ran = false;
        for (position, skill) in skills.into_iter().enumerate() {
            let cost_modifier = crate::engine::skill::buff_act::device_cost_reduce::modifier(
                managers,
                source_uid,
                skill.skill_id,
            );
            let cost_reduction = cost_modifier
                .as_ref()
                .map(|(reduction, _)| *reduction)
                .unwrap_or_default();
            if !managers
                .conduit
                .can_begin_skill(source_uid, skill.skill_id, cost_reduction)
            {
                continue;
            }
            append(
                &mut result,
                drain::run_conduit_action(
                    managers,
                    pool,
                    catalog,
                    determinism,
                    context,
                    source_uid,
                    group,
                    position as i32 + 1,
                    skill.skill_id,
                    cost_modifier,
                )?,
            );
            ran = true;
            if crate::engine::round::outcome::battle_ended(fight, pool, managers) {
                break;
            }
        }
        if ran {
            append(
                &mut result,
                drain::run_conduit_stop(
                    managers,
                    pool,
                    catalog,
                    determinism,
                    context,
                    source_uid,
                    group,
                )?,
            );
        }
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
/// Decodes and executes the player's authoritative queued card actions.
pub fn run_player_action_queue(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    plays: impl IntoIterator<Item = crate::engine::manager::card::CardPlay>,
    team: i32,
    emitter_uid: i64,
) -> Result<DrainResult, DrainError> {
    run_player_card_ops(
        managers,
        pool,
        catalog,
        determinism,
        context,
        plays.into_iter().map(PlayerCardOp::Play),
        team,
        emitter_uid,
        None,
    )
}

fn player_ops_from_commands(
    commands: impl IntoIterator<Item = RoundCommand>,
    determinism: &mut RoundDeterminism,
) -> Vec<PlayerCardOp> {
    commands
        .into_iter()
        .map(|command| match command {
            RoundCommand::MoveCard {
                from_index,
                to_index,
            } => PlayerCardOp::Move {
                from_index,
                to_index,
            },
            RoundCommand::UseUniversal {
                universal_index,
                target_index,
            } => PlayerCardOp::UseUniversal {
                universal_index,
                target_index,
            },
            RoundCommand::DissolveCard { card_index } => PlayerCardOp::Dissolve { card_index },
            RoundCommand::UseAssistBoss {
                skill_id,
                target_uid,
            } => PlayerCardOp::AssistBoss {
                skill_id,
                target_uid,
            },
            RoundCommand::PlayCard {
                card_index,
                target_uid,
                chosen_skill_id,
                recorded_skill,
            } => PlayerCardOp::Play(crate::engine::manager::card::CardPlay {
                origin: CARD_PLAY_ORIGIN,
                hand_index: card_index,
                target_uid,
                chosen_skill_id,
                choice: determinism.take_card_play(),
                recorded_skill,
            }),
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub fn run_player_commands(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    commands: impl IntoIterator<Item = RoundCommand>,
    team: i32,
    emitter_uid: i64,
) -> Result<DrainResult, DrainError> {
    let ops = player_ops_from_commands(commands, determinism);
    run_player_card_ops(
        managers,
        pool,
        catalog,
        determinism,
        context,
        ops,
        team,
        emitter_uid,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_player_phase(
    fight: &sonettobuf::Fight,
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    commands: impl IntoIterator<Item = RoundCommand>,
    team: i32,
    emitter_uid: i64,
) -> Result<DrainResult, DrainError> {
    let context = TargetContext {
        battle_id: fight.battle_id.unwrap_or_default(),
        ..context
    };
    let ops = player_ops_from_commands(commands, determinism);
    let mut result = run_player_card_ops(
        managers,
        pool,
        catalog,
        determinism,
        context,
        ops,
        team,
        emitter_uid,
        Some(fight),
    )?;
    append(
        &mut result,
        run_impromptu(
            managers,
            pool,
            catalog,
            determinism,
            context,
            team,
            emitter_uid,
        )?,
    );
    if crate::engine::round::outcome::battle_ended(fight, pool, managers) {
        return Ok(result);
    }
    Ok(result)
}

#[allow(clippy::large_enum_variant)]
enum PlayerCardOp {
    Move {
        from_index: usize,
        to_index: usize,
    },
    UseUniversal {
        universal_index: usize,
        target_index: usize,
    },
    Dissolve {
        card_index: usize,
    },
    AssistBoss {
        skill_id: i32,
        target_uid: Option<i64>,
    },
    Play(crate::engine::manager::card::CardPlay),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayerActionSource {
    Card,
    Queued,
    AssistBoss,
}

struct QueuedPlayerAction {
    skill: crate::engine::skill::action::SkillInvocation,
    source: PlayerActionSource,
    grants_ex_point: bool,
    grant_after_action: bool,
    composed_owners: Vec<(i64, i32)>,
    prelude: Vec<RuleOp>,
}

pub(crate) fn card_skill_is_blocked(
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    owner_uid: i64,
    skill_id: i32,
) -> bool {
    let effect_tag = catalog.effect_tag(skill_id);
    let is_big_skill = catalog.is_big_skill(skill_id);
    managers
        .buff
        .has_buff_act_kind(owner_uid, buff_act::registry::BuffActKind::Sleep)
        || (managers
            .buff
            .has_buff_act_kind(owner_uid, buff_act::registry::BuffActKind::Forbid)
            && buff_act::forbid::blocks(effect_tag, is_big_skill))
        || (managers
            .buff
            .has_buff_act_kind(owner_uid, buff_act::registry::BuffActKind::Disarm)
            && buff_act::disarm::blocks(effect_tag, is_big_skill))
        || (managers
            .buff
            .has_buff_act_kind(owner_uid, buff_act::registry::BuffActKind::Seal)
            && is_big_skill)
        || managers
            .buff
            .has_buff_act_kind(owner_uid, buff_act::registry::BuffActKind::CastChannel)
}

#[allow(clippy::too_many_arguments)]
fn run_player_card_ops(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    ops: impl IntoIterator<Item = PlayerCardOp>,
    team: i32,
    emitter_uid: i64,
    fight: Option<&sonettobuf::Fight>,
) -> Result<DrainResult, DrainError> {
    let mut result = DrainResult::default();
    let mut skills = Vec::new();
    let mut pending_rewards = Vec::new();
    for op in ops {
        let skill_count = skills.len();
        if let PlayerCardOp::AssistBoss {
            skill_id,
            target_uid,
        } = op
        {
            let fight = fight.ok_or(DrainError::MissingAssistBoss)?;
            let team = fight
                .attacker
                .as_ref()
                .ok_or(DrainError::MissingAssistBoss)?;
            let source_uid = team
                .assist_boss
                .as_ref()
                .and_then(|boss| boss.uid)
                .ok_or(DrainError::MissingAssistBoss)?;
            let skill = team
                .assist_boss_info
                .as_ref()
                .and_then(|info| {
                    info.skills
                        .iter()
                        .find(|skill| skill.skill_id == Some(skill_id))
                })
                .ok_or(DrainError::InvalidAssistBossSkill(skill_id))?;
            let cost = skill.need_power.unwrap_or_default().max(0);
            if managers
                .eureka
                .get(source_uid, PowerType::AssistBoss.id())
                .current
                < cost
            {
                return Err(DrainError::InsufficientAssistBossPower(skill_id));
            }
            let mut invocation: crate::engine::skill::action::SkillInvocation =
                crate::engine::skill::action::SkillRequest {
                    source_uid,
                    skill_id,
                }
                .into();
            if let Some(target_uid) = target_uid {
                invocation.target = crate::engine::skill::action::SkillTarget::Explicit(target_uid);
            }
            let prelude = (cost > 0)
                .then(|| {
                    RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(EurekaChange {
                        origin: CARD_PLAY_ORIGIN,
                        source_uid,
                        target_uid: source_uid,
                        power_id: PowerType::AssistBoss.id(),
                        delta: -cost,
                        effect_type: sonettobuf::effect_type_enum::EffectType::Powerchange as i32,
                    })))
                })
                .into_iter()
                .collect();
            skills.push(QueuedPlayerAction {
                skill: invocation,
                source: PlayerActionSource::AssistBoss,
                grants_ex_point: false,
                grant_after_action: false,
                composed_owners: Vec::new(),
                prelude,
            });
            continue;
        }
        if let PlayerCardOp::Play(play) = &op
            && let Some((owner_uid, skill_id)) =
                play.planned_skill(managers.card.visible_card(play.hand_index))
            && card_skill_is_blocked(managers, catalog, owner_uid, skill_id)
        {
            return Err(DrainError::ForbiddenCardSkill {
                owner_uid,
                skill_id,
            });
        }
        let moved_ex_point = match &op {
            PlayerCardOp::Move { from_index, .. } => managers
                .card
                .hand()
                .get(*from_index)
                .and_then(|card| card.uid)
                .filter(|uid| managers.entity.team_type(*uid) == Some(team))
                .and_then(|owner_uid| {
                    let delta = 1_i32.saturating_add(managers.buff.buff_act_scalar(
                        owner_uid,
                        crate::engine::skill::buff_act::registry::BuffActKind::ExPointCardMove,
                    ));
                    (delta > 0).then_some((owner_uid, delta))
                }),
            _ => None,
        };
        let command = match op {
            PlayerCardOp::Move {
                from_index,
                to_index,
            } => CardCommand::Move {
                origin: CARD_PLAY_ORIGIN,
                from_index,
                to_index,
            },
            PlayerCardOp::UseUniversal {
                universal_index,
                target_index,
            } => CardCommand::UseUniversal(crate::engine::manager::card::CardUseUniversal {
                origin: CARD_PLAY_ORIGIN,
                universal_index,
                target_index,
            }),
            PlayerCardOp::Dissolve { card_index } => CardCommand::Dissolve {
                origin: CARD_PLAY_ORIGIN,
                card_index,
            },
            PlayerCardOp::AssistBoss { .. } => unreachable!("handled before card commands"),
            PlayerCardOp::Play(play) => CardCommand::Play(play),
        };

        let played = drain::run(
            managers,
            pool,
            catalog,
            determinism,
            context,
            [RuleOp::Command(BattleCommand::Card(command))],
        )?;
        skills.extend(played.outcomes.iter().filter_map(|outcome| {
            let RuleOutcome::Card(changes) = outcome else {
                return None;
            };
            let played = changes.played.as_ref()?;
            let mut invocation: crate::engine::skill::action::SkillInvocation =
                crate::engine::skill::action::SkillRequest {
                    source_uid: played.caster_uid,
                    skill_id: played.skill_id,
                }
                .into();
            invocation.card_index = played.card_index;
            invocation.card_enchants = played
                .card
                .enchants
                .iter()
                .filter_map(|enchant| enchant.enchant_id)
                .collect();
            invocation.recorded_skill = played.recorded_skill;
            if let Some(target_uid) = played.target_uid {
                invocation.target = crate::engine::skill::action::SkillTarget::Explicit(target_uid);
            }
            let grants_ex_point = managers.entity.team_type(played.caster_uid) == Some(team)
                && managers.ex_point.kind(played.caster_uid) == 0
                && catalog.grants_resource_on_card_play(played.skill_id)
                && !crate::engine::manager::card::deck::has_enchant_type(
                    &played.card,
                    crate::engine::manager::card::EnchantedType::Lorenz,
                );
            Some(QueuedPlayerAction {
                skill: invocation,
                source: PlayerActionSource::Card,
                grants_ex_point,
                grant_after_action: played.card.temp_card.unwrap_or_default(),
                composed_owners: Vec::new(),
                prelude: Vec::new(),
            })
        }));
        append(&mut result, played);
        if let Some((owner_uid, delta)) = moved_ex_point {
            pending_rewards.push((owner_uid, delta));
        }
        let composed = drain::run(
            managers,
            pool,
            catalog,
            determinism,
            context,
            [RuleOp::Command(BattleCommand::Card(
                CardCommand::ComposeAdjacent {
                    origin: CARD_PLAY_ORIGIN,
                },
            ))],
        )?;
        let composed_owners = composed
            .outcomes
            .iter()
            .filter_map(|outcome| match outcome {
                RuleOutcome::Card(changes) => Some(changes.composed_owners.as_slice()),
                _ => None,
            })
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        append(&mut result, composed);
        if skills.len() > skill_count {
            skills
                .last_mut()
                .expect("a played card records its skill")
                .composed_owners
                .extend(
                    pending_rewards
                        .drain(..)
                        .chain(composed_owners.into_iter().map(|owner_uid| (owner_uid, 1))),
                );
        } else {
            pending_rewards.extend(composed_owners.into_iter().map(|owner_uid| (owner_uid, 1)));
        }
    }
    if skills.is_empty() {
        append(
            &mut result,
            run_card_composition_rewards(
                managers,
                pool,
                catalog,
                determinism,
                context,
                CARD_PLAY_ORIGIN,
                pending_rewards,
            )?,
        );
        return Ok(result);
    }
    if !pending_rewards.is_empty() {
        skills
            .last_mut()
            .expect("a non-empty action queue has a final skill")
            .composed_owners
            .extend(pending_rewards);
    }
    let committed = run_action_queue_committed(
        managers,
        pool,
        catalog,
        determinism,
        context,
        team,
        emitter_uid,
    )?;
    skills.extend(committed.outcomes.iter().filter_map(|outcome| {
        let RuleOutcome::Card(changes) = outcome else {
            return None;
        };
        let action = changes.queued_use_card.as_ref()?.action.clone()?;
        Some(QueuedPlayerAction {
            skill: action,
            source: PlayerActionSource::Queued,
            grants_ex_point: false,
            grant_after_action: false,
            composed_owners: Vec::new(),
            prelude: Vec::new(),
        })
    }));
    append(&mut result, committed);
    let queue_preparation = queue_preparation_ops(managers, pool, catalog, context, &skills);
    if !queue_preparation.is_empty() {
        append(
            &mut result,
            drain::run(
                managers,
                pool,
                catalog,
                determinism,
                context,
                queue_preparation,
            )?,
        );
    }
    append(
        &mut result,
        drain::run(
            managers,
            pool,
            catalog,
            determinism,
            context,
            [RuleOp::Command(BattleCommand::Card(
                CardCommand::ResolvePlayedRanks {
                    origin: CARD_PLAY_ORIGIN,
                },
            ))],
        )?,
    );
    for queued in skills {
        if fight
            .is_some_and(|fight| crate::engine::round::outcome::battle_ended(fight, pool, managers))
        {
            break;
        }
        let QueuedPlayerAction {
            mut skill,
            source,
            grants_ex_point,
            grant_after_action,
            composed_owners,
            prelude,
        } = queued;
        let is_card_action = source == PlayerActionSource::Card;
        if let Some(played) = managers
            .card
            .played()
            .iter()
            .find(|played| played.card_index == skill.card_index)
        {
            skill.plan.skill_id = played.skill_id;
        }
        let source_uid = skill.plan.source_uid;
        let source_alive = !is_card_action || managers.hp.current(source_uid) > 0;
        let is_ultimate = pool.entity(source_uid).is_some_and(|entity| {
            crate::engine::mechanic::card::CardMechanic
                .is_ultimate_skill(skill.plan.skill_id, entity)
        });
        if source_alive
            && let crate::engine::skill::action::SkillTarget::Explicit(target_uid) = skill.target
            && managers.hp.current(target_uid) <= 0
            && let Some(replacement_uid) =
                crate::engine::skill::target::TargetResolver::retarget_stale_explicit(
                    source_uid, target_uid, pool, managers,
                )
        {
            skill.target = crate::engine::skill::action::SkillTarget::Explicit(replacement_uid);
        }
        let configured_targets_enemy =
            crate::engine::skill::target::targets_enemy(catalog.logic_target(skill.plan.skill_id));
        let explicit_targets_enemy = match skill.target {
            crate::engine::skill::action::SkillTarget::Explicit(target_uid) => pool
                .team_type(source_uid)
                .zip(pool.team_type(target_uid))
                .map(|(source_team, target_team)| source_team != target_team),
            crate::engine::skill::action::SkillTarget::Inherited
            | crate::engine::skill::action::SkillTarget::Configured => None,
        };
        let targets_enemy = configured_targets_enemy
            .or(explicit_targets_enemy)
            .unwrap_or_else(|| catalog.is_attack(skill.plan.skill_id));
        let target_alive = match (configured_targets_enemy, skill.target) {
            (None, crate::engine::skill::action::SkillTarget::Explicit(target_uid)) => {
                managers.hp.current(target_uid) > 0
            }
            (Some(_), _)
            | (None, crate::engine::skill::action::SkillTarget::Inherited)
            | (None, crate::engine::skill::action::SkillTarget::Configured) => true,
        };
        let current_opponents = pool.enemies(source_uid, false);
        let current_opponents_defeated = !current_opponents.is_empty()
            && current_opponents
                .iter()
                .all(|enemy| managers.hp.current(enemy.uid) <= 0);
        if !source_alive || !target_alive || current_opponents_defeated && targets_enemy {
            let mut rewards = composed_owners;
            if !grant_after_action
                && let Some(delta) =
                    card_play_resource_delta(managers, source_uid, grants_ex_point, is_ultimate)
            {
                rewards.push((source_uid, delta));
            }
            if is_card_action {
                let restore = source_alive && is_ultimate;
                push_attributed_cue(
                    &mut result.frames,
                    source_uid,
                    RoundCue::CardInvalid {
                        card_index: skill.card_index,
                        team_type: team,
                        reason: if current_opponents_defeated && targets_enemy {
                            CardInvalidReason::OpponentsDefeated
                        } else {
                            CardInvalidReason::Default
                        },
                    },
                );
                append(
                    &mut result,
                    drain::run(
                        managers,
                        pool,
                        catalog,
                        determinism,
                        context,
                        [RuleOp::Command(BattleCommand::Card(
                            CardCommand::InvalidatePlayed(CardInvalidatePlayed {
                                origin: CARD_PLAY_ORIGIN,
                                card_index: skill.card_index,
                                restore,
                            }),
                        ))],
                    )?,
                );
            }
            append(
                &mut result,
                run_card_composition_rewards(
                    managers,
                    pool,
                    catalog,
                    determinism,
                    context,
                    CARD_PLAY_ORIGIN,
                    rewards,
                )?,
            );
            continue;
        }
        let (matching_rewards, other_rewards): (Vec<_>, Vec<_>) = composed_owners
            .into_iter()
            .partition(|(owner_uid, _)| *owner_uid == source_uid);
        let queued_resource_delta = matching_rewards
            .into_iter()
            .map(|(_, delta)| delta)
            .fold(0, i32::saturating_add);
        append(
            &mut result,
            run_card_composition_rewards(
                managers,
                pool,
                catalog,
                determinism,
                context,
                CARD_PLAY_ORIGIN,
                other_rewards,
            )?,
        );
        append(
            &mut result,
            run_active_action(
                managers,
                pool,
                catalog,
                determinism,
                context,
                ActiveActionRequest {
                    skill,
                    grants_ex_point,
                    grant_after_action,
                    queued_resource_delta,
                    prelude,
                },
            )?,
        );
    }
    Ok(result)
}

fn queue_preparation_ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    context: TargetContext,
    actions: &[QueuedPlayerAction],
) -> Vec<RuleOp> {
    actions
        .iter()
        .filter(|action| action.source == PlayerActionSource::Card)
        .flat_map(|action| {
            let skill_id = action.skill.plan.skill_id;
            let source_uid = action.skill.plan.source_uid;
            catalog.get(skill_id).into_iter().flat_map(move |effect| {
                effect.slots.iter().flat_map(move |slot| {
                    let Some(definition) =
                        crate::engine::skill::behavior::registry::find(&slot.behavior)
                    else {
                        return Vec::new().into_iter();
                    };
                    let Some(collect) = definition.collect_queue_preparation else {
                        return Vec::new().into_iter();
                    };
                    let Ok(setup_keys) = slot.compiled_setup_keys(SetupStage::GeneratedCard, 0)
                    else {
                        return Vec::new().into_iter();
                    };
                    if setup_keys.is_empty() {
                        return Vec::new().into_iter();
                    }
                    let conditions =
                        setup_keys
                            .into_iter()
                            .fold(slot.conditions.clone(), |conditions, key| {
                                crate::engine::skill::condition::satisfied_conditions(
                                    &conditions,
                                    key,
                                )
                            });
                    let repeats = crate::engine::skill::condition::conditions_fire_count(
                        &conditions,
                        source_uid,
                        &[source_uid],
                        Some(managers),
                        pool,
                        TargetContext {
                            active_skill_id: skill_id,
                            active_skill_source_uid: source_uid,
                            active_card_index: action.skill.card_index,
                            ..context
                        },
                    );
                    (0..repeats)
                        .flat_map(|_| {
                            collect(action.skill.card_index, &slot.behavior).unwrap_or_default()
                        })
                        .collect::<Vec<_>>()
                        .into_iter()
                })
            })
        })
        .collect()
}

pub(super) fn run_card_composition_rewards(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    origin: CommandOrigin,
    rewards: impl IntoIterator<Item = (i64, i32)>,
) -> Result<DrainResult, DrainError> {
    let rewards = rewards.into_iter().fold(
        Vec::<(i64, i32)>::new(),
        |mut combined, (owner_uid, delta)| {
            if let Some((_, current)) = combined.iter_mut().find(|(uid, _)| *uid == owner_uid) {
                *current = current.saturating_add(delta);
            } else {
                combined.push((owner_uid, delta));
            }
            combined
        },
    );
    let ops = rewards.into_iter().map(|(owner_uid, delta)| {
        RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(
            ExPointChange {
                origin,
                source_uid: owner_uid,
                target_uid: owner_uid,
                delta,
                config_effect: 0,
                effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
            },
        )))
    });
    drain::run_command_group(managers, pool, catalog, determinism, context, ops)
}

pub(super) fn card_play_resource_delta(
    managers: &BattleManagers,
    source_uid: i64,
    grants_ex_point: bool,
    is_ultimate: bool,
) -> Option<i32> {
    (grants_ex_point && !is_ultimate).then(|| {
        1 + managers
            .buff
            .buff_act_scalar(
                source_uid,
                crate::engine::skill::buff_act::registry::BuffActKind::UseCardFixExPoint,
            )
            .max(0)
    })
}

pub(super) struct ActiveActionRequest {
    pub(super) skill: crate::engine::skill::action::SkillInvocation,
    pub(super) grants_ex_point: bool,
    pub(super) grant_after_action: bool,
    pub(super) queued_resource_delta: i32,
    pub(super) prelude: Vec<RuleOp>,
}

pub(super) fn run_active_action(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    request: ActiveActionRequest,
) -> Result<DrainResult, DrainError> {
    let ActiveActionRequest {
        mut skill,
        grants_ex_point,
        grant_after_action,
        queued_resource_delta,
        prelude,
    } = request;
    let mut result = DrainResult::default();
    if skill.mode == crate::engine::skill::action::SkillExecutionMode::Nested {
        skill.mode = crate::engine::skill::action::SkillExecutionMode::Active;
    }
    let source_uid = skill.plan.source_uid;
    let is_ultimate = pool.entity(source_uid).is_some_and(|entity| {
        crate::engine::mechanic::card::CardMechanic.is_ultimate_skill(skill.plan.skill_id, entity)
    });
    let current_resource = managers.ex_point.get(source_uid);
    let ultimate_cost = if is_ultimate {
        let kind = crate::engine::manager::ex_point::ExPointKind::from_wire(
            managers.ex_point.kind(source_uid),
        );
        if kind == crate::engine::manager::ex_point::ExPointKind::Common {
            pool.entity(source_uid)
                .map(|entity| {
                    crate::engine::mechanic::card::CardMechanic
                        .required_ultimate_cost(managers, entity)
                })
                .unwrap_or(current_resource)
        } else {
            current_resource
        }
    } else {
        0
    };
    if is_ultimate && current_resource < ultimate_cost {
        return Err(DrainError::InsufficientUltimateResource {
            owner_uid: source_uid,
            skill_id: skill.plan.skill_id,
            required: ultimate_cost,
            current: current_resource,
        });
    }
    let resource_delta =
        card_play_resource_delta(managers, source_uid, grants_ex_point, is_ultimate);
    let pre_action_delta = queued_resource_delta.saturating_add(
        resource_delta
            .filter(|_| !grant_after_action)
            .unwrap_or_default(),
    );
    if pre_action_delta > 0 {
        append(
            &mut result,
            drain::run(
                managers,
                pool,
                catalog,
                determinism,
                context,
                [RuleOp::Command(BattleCommand::ExPoint(
                    ExPointCommand::Change(ExPointChange {
                        origin: CARD_PLAY_ORIGIN,
                        source_uid,
                        target_uid: source_uid,
                        delta: pre_action_delta,
                        config_effect: 0,
                        effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
                    }),
                ))],
            )?,
        );
    }
    let ultimate_moxie = ultimate_cost.max(0);
    let action_cost = (ultimate_moxie > 0).then(|| {
        ExPointCommand::Change(ExPointChange {
            origin: CARD_PLAY_ORIGIN,
            source_uid,
            target_uid: source_uid,
            delta: -ultimate_moxie,
            config_effect: 0,
            effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
        })
    });
    append(
        &mut result,
        drain::run_action_with_cost(
            managers,
            pool,
            catalog,
            determinism,
            context,
            prelude,
            action_cost,
            skill,
        )?,
    );
    if let Some(delta) = resource_delta.filter(|_| grant_after_action) {
        append(
            &mut result,
            drain::run(
                managers,
                pool,
                catalog,
                determinism,
                context,
                [RuleOp::Command(BattleCommand::ExPoint(
                    ExPointCommand::Change(ExPointChange {
                        origin: CARD_PLAY_ORIGIN,
                        source_uid,
                        target_uid: source_uid,
                        delta,
                        config_effect: 0,
                        effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
                    }),
                ))],
            )?,
        );
    }
    Ok(result)
}
