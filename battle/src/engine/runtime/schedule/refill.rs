use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RefillStage {
    Opening,
    AfterActions,
    RoundStart,
}

/// Refills the normal hand deficit, then resolves composition and replacement rules.
pub fn run_round_refill(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    hand_size: usize,
    team_type: i32,
) -> Result<DrainResult, DrainError> {
    run_card_refill(
        managers,
        pool,
        catalog,
        determinism,
        context,
        hand_size,
        team_type,
        RefillStage::AfterActions,
        Vec::new(),
    )
}

pub(super) fn run_round_start_refill(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    hand_size: usize,
    team_type: i32,
) -> Result<DrainResult, DrainError> {
    run_card_refill(
        managers,
        pool,
        catalog,
        determinism,
        context,
        hand_size,
        team_type,
        RefillStage::RoundStart,
        Vec::new(),
    )
}

pub(super) fn run_opening_hand_refill(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    hand_size: usize,
    opening_draws: Vec<sonettobuf::CardInfo>,
) -> Result<DrainResult, DrainError> {
    run_card_refill(
        managers,
        pool,
        catalog,
        determinism,
        context,
        hand_size,
        1,
        RefillStage::Opening,
        opening_draws,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_card_refill(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    hand_size: usize,
    team_type: i32,
    stage: RefillStage,
    opening_draws: Vec<sonettobuf::CardInfo>,
) -> Result<DrainResult, DrainError> {
    let mut opening_draws = opening_draws.into_iter();
    let mut result = begin_round_phase(RoundPhase::CardRefill);
    let origin = CommandOrigin {
        domain: RuleDomain::Lifecycle,
        key: DefinitionKey::new(0, "RoundRefill"),
    };
    if stage == RefillStage::AfterActions {
        append_round_phase(
            &mut result,
            drain::run(
                managers,
                pool,
                catalog,
                determinism,
                context,
                [RuleOp::Command(BattleCommand::Card(
                    CardCommand::ExpireTemporary { origin },
                ))],
            )?,
        );
    }
    let composition = drain::run(
        managers,
        pool,
        catalog,
        determinism,
        context,
        [RuleOp::Command(BattleCommand::Card(
            CardCommand::ComposeAdjacent { origin },
        ))],
    )?;
    let initial_composed = composition
        .outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            RuleOutcome::Card(changes) => Some(changes.composed_owners.as_slice()),
            _ => None,
        })
        .flatten()
        .copied()
        .filter(|owner_uid| managers.ex_point.gains_composition_ex_point(*owner_uid))
        .collect::<Vec<_>>();
    append_round_phase(
        &mut result,
        run_card_composition_rewards(
            managers,
            pool,
            catalog,
            determinism,
            context,
            origin,
            initial_composed.into_iter().map(|owner_uid| (owner_uid, 1)),
        )?,
    );
    append_round_phase(&mut result, composition);
    loop {
        let ready_normal = if stage != RefillStage::Opening {
            crate::engine::mechanic::card::CardMechanic.normal_ultimate_cards(pool, managers)
        } else {
            Vec::new()
        };
        let ready_special = if stage == RefillStage::AfterActions {
            crate::engine::mechanic::card::CardMechanic
                .special_team_cards(pool, managers, managers.card.hand())
                .into_iter()
                .filter(|candidate| {
                    !managers.card.team_cards().iter().any(|existing| {
                        existing.uid == candidate.uid && existing.skill_id == candidate.skill_id
                    })
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let needs_normal_card = match stage {
            RefillStage::Opening => managers.card.hand().len() < hand_size,
            RefillStage::AfterActions | RefillStage::RoundStart => {
                crate::engine::mechanic::card::CardMechanic.refill_hand_len(managers, pool)
                    < hand_size
            }
        };
        if !needs_normal_card
            && ready_normal.is_empty()
            && ready_special.is_empty()
            && opening_draws.len() == 0
        {
            break;
        }
        let ready_ultimates = pool
            .attacker_main
            .iter()
            .filter_map(|entity| {
                ready_normal
                    .iter()
                    .chain(&ready_special)
                    .find(|card| card.uid == Some(entity.uid))
                    .cloned()
            })
            .collect::<Vec<_>>();
        if needs_normal_card && ready_ultimates.is_empty() && managers.card.can_recycle_draw_pile()
        {
            let recycled = drain::run(
                managers,
                pool,
                catalog,
                determinism,
                context,
                [RuleOp::Command(BattleCommand::Card(
                    CardCommand::RecycleDrawPile { origin, team_type },
                ))],
            )?;
            append_round_phase(&mut result, recycled);
            continue;
        }
        let mut candidates = managers
            .card
            .draw_pile()
            .iter()
            .filter(|card| {
                pool.entity(card.uid.unwrap_or_default())
                    .is_none_or(|entity| {
                        !crate::engine::mechanic::card::CardMechanic.is_ultimate(card, entity)
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        for ready in &ready_ultimates {
            if !candidates
                .iter()
                .any(|candidate| candidate.uid == ready.uid && candidate.skill_id == ready.skill_id)
            {
                candidates.push(ready.clone());
            }
        }
        let (card, configured_opening) = if let Some(card) = opening_draws.next() {
            (Some(card), true)
        } else if !needs_normal_card {
            (ready_ultimates.first().cloned(), false)
        } else if determinism.has_queued_card_draw() {
            (determinism.draw_cards(&candidates, 1).pop(), false)
        } else {
            (
                ready_ultimates
                    .first()
                    .cloned()
                    .or_else(|| determinism.draw_cards(&candidates, 1).pop()),
                false,
            )
        };
        let Some(card) = card else {
            break;
        };
        let is_ultimate = pool
            .entity(card.uid.unwrap_or_default())
            .is_some_and(|entity| {
                crate::engine::mechanic::card::CardMechanic.is_ultimate(&card, entity)
            });
        let is_device = crate::engine::mechanic::card::CardMechanic.is_device_card(&card);
        if is_ultimate
            && !ready_ultimates
                .iter()
                .any(|ready| ready.uid == card.uid && ready.skill_id == card.skill_id)
        {
            continue;
        }
        if !needs_normal_card && !is_ultimate && !configured_opening {
            continue;
        }
        let refill = drain::run(
            managers,
            pool,
            catalog,
            determinism,
            context,
            [RuleOp::Command(BattleCommand::Card(
                CardCommand::RefillOne(CardRefillOne {
                    origin,
                    card,
                    consume_draw_pile: !configured_opening && !is_ultimate,
                    consume_deck: !configured_opening
                        && !is_ultimate
                        && !is_device
                        && managers.card.refill_consumes_deck(),
                }),
            ))],
        )?;
        let composed_owners = refill
            .outcomes
            .iter()
            .filter_map(|outcome| match outcome {
                RuleOutcome::Card(changes) => Some(changes.composed_owners.as_slice()),
                _ => None,
            })
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        let composed_count = composed_owners.len();
        append_round_phase(&mut result, refill);
        let reward_owners = composed_owners
            .into_iter()
            .filter(|owner_uid| managers.ex_point.gains_composition_ex_point(*owner_uid))
            .map(|owner_uid| (owner_uid, 1))
            .collect::<Vec<_>>();
        append_round_phase(
            &mut result,
            run_card_composition_rewards(
                managers,
                pool,
                catalog,
                determinism,
                context,
                origin,
                reward_owners,
            )?,
        );
        if composed_count != 0 {
            let mut cues = DrainResult::default();
            push_cues(
                &mut cues.frames,
                std::iter::repeat_n(RoundCue::CardsCompose { team_type }, composed_count),
            );
            append_round_phase(&mut result, cues);
        }
    }
    if stage == RefillStage::AfterActions {
        let mut summary = DrainResult::default();
        push_cues(
            &mut summary.frames,
            [RoundCue::DeckCount {
                count: managers.card.deck_num(),
                team_type,
            }],
        );
        append_round_phase(&mut result, summary);
    }
    Ok(result)
}

/// Emits the semantic round-deal cue without mutating card storage.
pub fn run_round_deal(team_type: i32) -> DrainResult {
    let mut result = DrainResult::default();
    push_cue(&mut result.frames, RoundCue::DealCards { team_type });
    result
}

pub fn run_ai_queue_refresh(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    cards: Vec<sonettobuf::CardInfo>,
) -> Result<DrainResult, DrainError> {
    let origin = CommandOrigin {
        domain: RuleDomain::Lifecycle,
        key: DefinitionKey::new(0, "AiQueueRefresh"),
    };
    let mut result = drain::run(
        managers,
        pool,
        catalog,
        determinism,
        context,
        [RuleOp::Command(BattleCommand::Card(
            CardCommand::RefreshAiQueue(CardRefreshAiQueue { origin, cards }),
        ))],
    )?;
    let composed_owners = result
        .outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            RuleOutcome::Card(changes) => Some(changes.composed_owners.as_slice()),
            _ => None,
        })
        .flatten()
        .copied()
        .filter(|owner_uid| managers.ex_point.gains_composition_ex_point(*owner_uid))
        .map(|owner_uid| (owner_uid, 1))
        .collect::<Vec<_>>();
    append(
        &mut result,
        run_card_composition_rewards(
            managers,
            pool,
            catalog,
            determinism,
            context,
            origin,
            composed_owners,
        )?,
    );
    Ok(result)
}

pub fn run_entity_card_cleanup(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    owner_uids: impl IntoIterator<Item = i64>,
) -> Result<DrainResult, DrainError> {
    let mut result = begin_round_phase(RoundPhase::EntityCardCleanup);
    for owner_uid in owner_uids {
        if managers.hp.current(owner_uid) > 0 {
            continue;
        }
        let Some(team_type) = pool
            .team_type(owner_uid)
            .or_else(|| managers.buff.team_type(owner_uid))
        else {
            continue;
        };
        let cleanup = drain::run_command_group(
            managers,
            pool,
            catalog,
            determinism,
            context,
            [RuleOp::Command(BattleCommand::Card(
                CardCommand::RemoveAiOwner(crate::engine::manager::card::CardRemoveAiOwner {
                    origin: CommandOrigin {
                        domain: RuleDomain::Lifecycle,
                        key: DefinitionKey::new(0, "EntityCardCleanup"),
                    },
                    owner_uid,
                    team_type,
                }),
            ))],
        )?;
        let composed_owners = cleanup
            .outcomes
            .iter()
            .filter_map(|outcome| match outcome {
                RuleOutcome::Card(changes) => Some(changes.composed_owners.as_slice()),
                _ => None,
            })
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        result.outcomes.extend(cleanup.outcomes);
        result.events.extend(cleanup.events);
        result
            .frames
            .first_mut()
            .expect("card-cleanup phase has a root")
            .items
            .extend(
                cleanup
                    .frames
                    .into_iter()
                    .map(|frame| FrameItem::Child(Box::new(frame))),
            );
        if composed_owners.is_empty() {
            continue;
        }
        let reward_owners = composed_owners
            .into_iter()
            .filter(|uid| managers.ex_point.gains_composition_ex_point(*uid))
            .map(|uid| (uid, 1))
            .collect::<Vec<_>>();
        append_round_phase(
            &mut result,
            run_card_composition_rewards(
                managers,
                pool,
                catalog,
                determinism,
                context,
                CARD_PLAY_ORIGIN,
                reward_owners,
            )?,
        );
    }
    Ok(result)
}
