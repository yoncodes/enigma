use super::*;

#[derive(Default)]
pub(super) struct RoundStartSettlementPlan {
    pub(super) capacity_groups: Vec<crate::engine::mechanic::shadow_cloak::CapacityRuleGroup>,
    pub(super) buff_owner_uids: Vec<i64>,
}

impl RoundStartSettlementPlan {
    pub(super) fn new(
        capacity_groups: Vec<crate::engine::mechanic::shadow_cloak::CapacityRuleGroup>,
        mut buff_owner_uids: Vec<i64>,
    ) -> Self {
        for owner_uid in capacity_groups.iter().map(|group| group.owner_uid) {
            if !buff_owner_uids.contains(&owner_uid) {
                buff_owner_uids.push(owner_uid);
            }
        }
        Self {
            capacity_groups,
            buff_owner_uids,
        }
    }
}

fn append_opening_settlement(settlement: &mut DrainResult, next: DrainResult, version7: bool) {
    if version7 {
        append_opening_round_phase(settlement, next);
    } else {
        append_round_phase(settlement, next);
    }
}

#[cfg(test)]
pub fn run_round_start_split(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    team_type: i32,
) -> Result<(DrainResult, DrainResult), DrainError> {
    let mut before =
        run_before_ai_round_start(managers, pool, catalog, determinism, context, team_type)?;
    let base_hand_size = crate::engine::manager::card::start::hand_size_from_count(
        pool.attacker_main
            .iter()
            .filter(|entity| managers.hp.current(entity.uid) > 0)
            .count(),
    );
    let hand_size = crate::engine::mechanic::card::CardMechanic.normal_hand_limit(
        base_hand_size,
        managers,
        pool,
    );
    let (after, next_round, _, _) = run_round_start_after_ai_split(
        managers,
        pool,
        catalog,
        determinism,
        context,
        &[],
        hand_size,
    )?;
    append(&mut before, after);
    Ok((before, next_round))
}

pub fn run_before_ai_round_start(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    _team_type: i32,
) -> Result<DrainResult, DrainError> {
    let mut owner_uids = pool
        .defender_main
        .iter()
        .filter(|entity| managers.hp.current(entity.uid) > 0)
        .map(|entity| entity.uid)
        .collect::<Vec<_>>();
    owner_uids.extend(pool.assist_boss(crate::engine::fight::rules::DEFENDER_SIDE_UID));
    let (result, pending_settlement) = run_round_start_before_duration(
        managers,
        pool,
        catalog,
        determinism,
        context,
        2,
        &owner_uids,
        false,
    )?;
    debug_assert!(pending_settlement.capacity_groups.is_empty());
    Ok(result)
}

pub fn run_round_start_after_ai_split(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    entering_uids: &[i64],
    hand_size: usize,
) -> Result<
    (
        DrainResult,
        DrainResult,
        Vec<sonettobuf::CardInfo>,
        Vec<sonettobuf::CardInfo>,
    ),
    DrainError,
> {
    let mut owner_uids = pool
        .attacker_main
        .iter()
        .filter(|entity| managers.hp.current(entity.uid) > 0)
        .map(|entity| entity.uid)
        .collect::<Vec<_>>();
    owner_uids.extend(pool.assist_boss(crate::engine::fight::rules::ATTACKER_SIDE_UID));
    let duration_snapshot = duration_snapshot(managers, &owner_uids);
    let mut fight_steps = DrainResult::default();
    push_cue(
        &mut fight_steps.frames,
        RoundCue::ChangeRound {
            round: if crate::engine::fight::versions::writes_change_round_number(
                managers.fight_version(),
            ) {
                context.current_round
            } else {
                0
            },
        },
    );
    if !entering_uids.is_empty() {
        append(
            &mut fight_steps,
            run_wave_entry_setup(managers, pool, catalog, determinism, context, entering_uids)?,
        );
    }
    let (before_duration, settlement_plan) = run_round_start_before_duration(
        managers,
        pool,
        catalog,
        determinism,
        context,
        1,
        &owner_uids,
        true,
    )?;
    append(&mut fight_steps, before_duration);
    append(
        &mut fight_steps,
        run_round_start_damage_heal_settlement(managers, pool, catalog, determinism, context)?,
    );
    let mut settlement = begin_round_phase(RoundPhase::RoundStartSettlement);
    append_round_phase(
        &mut settlement,
        drain::run_buff_act_setup_stage_for_owners(
            managers,
            pool,
            catalog,
            determinism,
            context,
            SetupStage::RoundStart,
            2,
            &owner_uids,
        )?,
    );
    append_round_phase(
        &mut settlement,
        run_round_start_after_loss_mechanics(
            managers,
            pool,
            catalog,
            determinism,
            context,
            &owner_uids,
            settlement_plan,
        )?,
    );
    let duration = drain::run(
        managers,
        pool,
        catalog,
        determinism,
        context,
        duration_advance_rule(effect_time::ROUND_START_DURATION, &duration_snapshot),
    )?;
    append_round_phase(&mut settlement, duration);
    let (event_setup, independent_setup) = run_round_start_after_duration_setup(
        managers,
        pool,
        catalog,
        determinism,
        context,
        &owner_uids,
    )?;
    let setup_layout =
        crate::engine::fight::versions::round_start_setup_layout(managers.fight_version());
    if setup_layout == Some(crate::engine::fight::versions::RoundStartSetupLayout::Version7) {
        append_round_phase(
            &mut settlement,
            drain::run_setup_schedule(
                managers,
                pool,
                catalog,
                determinism,
                context,
                ROUND_START_THRESHOLD_SETUP,
            )?,
        );
    }
    append_round_phase(&mut settlement, event_setup);
    let settlement_setup = drain::run_setup_schedule(
        managers,
        pool,
        catalog,
        determinism,
        context,
        ROUND_START_SETTLEMENT_SETUP,
    )?;
    append_round_phase(&mut settlement, settlement_setup);
    append(&mut fight_steps, settlement);
    append(&mut fight_steps, independent_setup);
    let sync_schedule = match setup_layout {
        Some(crate::engine::fight::versions::RoundStartSetupLayout::Version7) => {
            ROUND_START_VERSION7_SYNC_SETUP
        }
        _ => ROUND_START_VERSION6_SYNC_SETUP,
    };
    let mut sync_setup = begin_round_phase(RoundPhase::RoundStartSync);
    append_round_phase(
        &mut sync_setup,
        drain::run_setup_schedule_in_owner_order_round_phase(
            managers,
            pool,
            catalog,
            determinism,
            context,
            sync_schedule,
            &owner_uids,
        )?,
    );
    append(&mut fight_steps, sync_setup);
    append(
        &mut fight_steps,
        drain::run_setup_stage_for_owners(
            managers,
            pool,
            catalog,
            determinism,
            context,
            SetupStage::RoundStartLate,
            0,
            &owner_uids,
        )?,
    );
    let defeated_defenders = pool
        .defender_all
        .iter()
        .filter(|entity| managers.hp.current(entity.uid) <= 0)
        .map(|entity| entity.uid)
        .collect::<Vec<_>>();
    append(
        &mut fight_steps,
        run_entity_card_cleanup(
            managers,
            pool,
            catalog,
            determinism,
            context,
            defeated_defenders,
        )?,
    );
    if setup_layout == Some(crate::engine::fight::versions::RoundStartSetupLayout::Version7) {
        append(
            &mut fight_steps,
            run_action_phase_start(managers, pool, catalog, determinism, context, 1)?,
        );
    }
    push_cue(
        &mut fight_steps.frames,
        RoundCue::DeckCount {
            count: managers.card.deck_num(),
            team_type: 1,
        },
    );
    let hand_snapshot = managers.card.hand().to_vec();
    let mut next_round_begin_steps = DrainResult::default();
    push_cue(&mut next_round_begin_steps.frames, RoundCue::DealCard1);
    push_cue(
        &mut next_round_begin_steps.frames,
        RoundCue::LayerHaloSync {
            buffs: managers.buff.layer_halo_sync(),
        },
    );
    append(
        &mut next_round_begin_steps,
        drain::run_setup_stage_with_prelude(
            managers,
            pool,
            catalog,
            determinism,
            context,
            SetupStage::AfterRoundStart,
            0,
            [(
                SetupSide::Attacker,
                RuleOp::Command(BattleCommand::Buff(BuffCommand::CleanupRoundStart(
                    BuffRoundStartCleanup::new(),
                ))),
            )],
        )?,
    );
    append(
        &mut next_round_begin_steps,
        drain::run_group_event(
            managers,
            pool,
            catalog,
            determinism,
            context,
            BattleEvent::Kind(EventKind::RoundStartCard),
            drain::ReactionLane::BuffActs,
            Some(&owner_uids),
        )?,
    );
    append(
        &mut next_round_begin_steps,
        run_card_energy_allocation(managers, pool, catalog, determinism, context, 1)?,
    );
    append(
        &mut next_round_begin_steps,
        drain::run_setup_stage(
            managers,
            pool,
            catalog,
            determinism,
            context,
            SetupStage::CardSetup,
            0,
        )?,
    );
    let refill_start = managers.card.refilled().len();
    append(
        &mut next_round_begin_steps,
        run_round_start_refill(managers, pool, catalog, determinism, context, hand_size, 1)?,
    );
    let mut dealt_cards = managers.card.refilled()[refill_start..].to_vec();
    for take_stage in effect_time::ROUND_START_CARD_STAGES {
        append(
            &mut next_round_begin_steps,
            drain::run(
                managers,
                pool,
                catalog,
                determinism,
                context,
                duration_advance_rule(take_stage, &duration_snapshot),
            )?,
        );
    }
    let team_cards = crate::engine::mechanic::card::CardMechanic.special_team_cards(
        pool,
        managers,
        managers.card.hand(),
    );
    append(
        &mut next_round_begin_steps,
        drain::run(
            managers,
            pool,
            catalog,
            determinism,
            context,
            [RuleOp::Command(BattleCommand::Card(
                CardCommand::SetTeamCards(crate::engine::manager::card::CardSetTeamCards {
                    origin: CommandOrigin {
                        domain: RuleDomain::Lifecycle,
                        key: DefinitionKey::new(0, "RoundStartTeamCards"),
                    },
                    cards: team_cards,
                }),
            ))],
        )?,
    );
    dealt_cards.extend_from_slice(managers.card.team_cards());
    let next_cards = managers
        .card
        .hand()
        .iter()
        .chain(managers.card.team_cards())
        .cloned()
        .collect();
    push_cue(
        &mut next_round_begin_steps.frames,
        RoundCue::NextRoundCards {
            cards: next_cards,
            deck_count: managers.card.deck_num(),
            team_type: 1,
        },
    );
    if managers.card.hand_limit_bonus() != 0 {
        append(
            &mut next_round_begin_steps,
            drain::run(
                managers,
                pool,
                catalog,
                determinism,
                context,
                [RuleOp::Command(BattleCommand::Card(
                    CardCommand::ClearHandLimit {
                        origin: HAND_LIMIT_CLEANUP_ORIGIN,
                    },
                ))],
            )?,
        );
    }
    Ok((
        fight_steps,
        next_round_begin_steps,
        hand_snapshot,
        dealt_cards,
    ))
}

fn duration_advance_rule(take_stage: i32, snapshot: &[(i64, i64)]) -> Option<RuleOp> {
    (!snapshot.is_empty()).then(|| {
        RuleOp::Command(BattleCommand::BuffBatch(
            snapshot
                .iter()
                .map(|(owner_uid, buff_uid)| {
                    BuffCommand::AdvanceDuration(
                        BuffDurationAdvance::new(
                            take_stage,
                            vec![*owner_uid],
                            Some(vec![*buff_uid]),
                        )
                        .expect("duration effect time is registered"),
                    )
                })
                .collect(),
        ))
    })
}

fn duration_snapshot(managers: &BattleManagers, owner_uids: &[i64]) -> Vec<(i64, i64)> {
    owner_uids
        .iter()
        .flat_map(|owner_uid| {
            let mut buff_uids = managers
                .buff
                .active_for(*owner_uid)
                .filter_map(|buff| buff.uid)
                .collect::<Vec<_>>();
            buff_uids.sort_unstable();
            buff_uids
                .into_iter()
                .map(|buff_uid| (*owner_uid, buff_uid))
                .collect::<Vec<_>>()
        })
        .collect()
}

pub fn run_finished_round_transition(managers: &BattleManagers) -> (DrainResult, DrainResult) {
    let mut fight_steps = DrainResult::default();
    push_cue(&mut fight_steps.frames, RoundCue::ChangeRound { round: 0 });

    let mut next_round_begin_steps = DrainResult::default();
    push_cue(
        &mut next_round_begin_steps.frames,
        RoundCue::NextRoundCards {
            cards: managers.card.hand().to_vec(),
            deck_count: managers.card.deck_num(),
            team_type: 1,
        },
    );
    (fight_steps, next_round_begin_steps)
}

pub fn run_start(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    card_setup: CardSetup,
    hand_size: usize,
) -> Result<(DrainResult, Vec<sonettobuf::CardInfo>), DrainError> {
    let mut result = DrainResult::default();
    let conduit_initializations = managers
        .conduit
        .initialization_commands()
        .into_iter()
        .map(|command| RuleOp::Command(BattleCommand::Conduit(command)))
        .collect::<Vec<_>>();
    if !conduit_initializations.is_empty() {
        append(
            &mut result,
            drain::run(
                managers,
                pool,
                catalog,
                determinism,
                context,
                conduit_initializations,
            )?,
        );
    }
    let mut card_setup = Some(card_setup);
    let mut dealt_cards = None;
    let mut opening_deck_counts = None;
    let mut opening_draws = Vec::new();
    let owner_uids = pool
        .attacker_main
        .iter()
        .filter(|entity| managers.hp.current(entity.uid) > 0)
        .map(|entity| entity.uid)
        .collect::<Vec<_>>();
    let existing_duration_snapshot = duration_snapshot(managers, &owner_uids);
    let mut opening_duration_snapshot = None;
    let mut opening_duration_captured = false;
    let mut opening_settlement = None;
    let version7_opening =
        crate::engine::fight::versions::round_start_setup_layout(managers.fight_version())
            == Some(crate::engine::fight::versions::RoundStartSetupLayout::Version7);
    for (stage, priority) in opening_setup(managers.fight_version()) {
        if stage == SetupStage::RoundStart && !opening_duration_captured {
            opening_duration_snapshot = Some(
                duration_snapshot(managers, &owner_uids)
                    .into_iter()
                    .filter(|entry| !existing_duration_snapshot.contains(entry))
                    .collect::<Vec<_>>(),
            );
            opening_duration_captured = true;
        }
        if version7_opening && stage == SetupStage::RoundStart && priority == 1 {
            let mut settlement = begin_round_phase(RoundPhase::RoundStartSettlement);
            let duration_snapshot = opening_duration_snapshot
                .take()
                .expect("opening schedule reaches round start before the late setup lane");
            append_round_phase(
                &mut settlement,
                drain::run(
                    managers,
                    pool,
                    catalog,
                    determinism,
                    context,
                    duration_advance_rule(effect_time::ROUND_START_DURATION, &duration_snapshot),
                )?,
            );
            opening_settlement = Some(settlement);
        }
        if stage == SetupStage::BuffGate {
            let mut settlement = opening_settlement
                .take()
                .expect("opening schedule runs the state reset before the buff gate");
            if !version7_opening {
                let duration_snapshot = opening_duration_snapshot
                    .take()
                    .expect("opening schedule reaches round start before the buff gate");
                append_round_phase(
                    &mut settlement,
                    drain::run(
                        managers,
                        pool,
                        catalog,
                        determinism,
                        context,
                        duration_advance_rule(
                            effect_time::ROUND_START_DURATION,
                            &duration_snapshot,
                        ),
                    )?,
                );
            }
            append_opening_settlement(
                &mut settlement,
                drain::run_setup_schedule_for_owners_in_opening_round_phase(
                    managers,
                    pool,
                    catalog,
                    determinism,
                    context,
                    &[(stage, priority)],
                    &owner_uids,
                )?,
                version7_opening,
            );
            let reset_ops = if version7_opening {
                managers.conduit.action_phase_start_commands(1)
            } else {
                managers.conduit.opening_reset_commands()
            }
            .into_iter()
            .map(|command| RuleOp::Command(BattleCommand::Conduit(command)))
            .collect::<Vec<_>>();
            if !reset_ops.is_empty() {
                append_round_phase(
                    &mut settlement,
                    drain::run(managers, pool, catalog, determinism, context, reset_ops)?,
                );
            }
            append(&mut result, settlement);
            continue;
        }
        if stage == SetupStage::CardSetup {
            let mut setup = card_setup
                .take()
                .expect("start schedule has one CardSetup stage");
            let card_mechanic = crate::engine::mechanic::card::CardMechanic;
            let free_deal_count = setup
                .hand
                .iter()
                .filter(|card| card_mechanic.counts_toward_hand_limit(card, managers, pool))
                .count();
            let hand_size = card_mechanic.normal_hand_limit(hand_size, managers, pool);
            let mut normal_cards = 0;
            setup.hand.retain(|card| {
                if !card_mechanic.counts_toward_hand_limit(card, managers, pool) {
                    return true;
                }
                normal_cards += 1;
                if normal_cards <= hand_size {
                    true
                } else {
                    opening_draws.push(card.clone());
                    false
                }
            });
            if free_deal_count < hand_size {
                setup
                    .hand
                    .extend(determinism.draw_cards(&setup.draw_pile, hand_size - free_deal_count));
            }
            let initial_deck_num = setup.deck_num;
            let supplemental = setup
                .hand
                .iter()
                .filter(|card| card_mechanic.counts_toward_hand_limit(card, managers, pool))
                .skip(free_deal_count)
                .cloned()
                .collect::<Vec<_>>();
            let mut consumed = 0_i32;
            for card in supplemental {
                let Some(index) = setup.draw_pile.iter().position(|candidate| {
                    candidate.uid == card.uid
                        && candidate.skill_id == card.skill_id
                        && candidate.temp_card == card.temp_card
                }) else {
                    continue;
                };
                setup.draw_pile.remove(index);
                if !card_mechanic.is_device_card(&card) {
                    consumed = consumed.saturating_add(1);
                }
            }
            setup.deck_num = setup.deck_num.saturating_sub(consumed);
            append(
                &mut result,
                drain::run(
                    managers,
                    pool,
                    catalog,
                    determinism,
                    context,
                    [RuleOp::Command(BattleCommand::Card(CardCommand::Setup(
                        setup,
                    )))],
                )?,
            );
            dealt_cards = Some(managers.card.hand().to_vec());
            push_cue(&mut result.frames, RoundCue::EnterFightDeal);
            opening_deck_counts = Some((initial_deck_num, managers.card.deck_num()));
        }
        let stage_result = if stage == SetupStage::RoundStart && priority == 2 {
            drain::run_buff_act_setup_stage_for_owners(
                managers,
                pool,
                catalog,
                determinism,
                context,
                stage,
                priority,
                &owner_uids,
            )?
        } else if version7_opening && stage == SetupStage::RoundStart && priority == 1 {
            drain::run_setup_schedule_in_opening_round_phase(
                managers,
                pool,
                catalog,
                determinism,
                context,
                &[(stage, priority)],
            )?
        } else if stage == SetupStage::AfterRoundStart {
            drain::run_setup_stage_with_prelude(
                managers,
                pool,
                catalog,
                determinism,
                context,
                stage,
                priority,
                [(
                    SetupSide::Attacker,
                    RuleOp::Command(BattleCommand::Buff(BuffCommand::CleanupRoundStart(
                        BuffRoundStartCleanup::new(),
                    ))),
                )],
            )?
        } else {
            drain::run_setup_stage(
                managers,
                pool,
                catalog,
                determinism,
                context,
                stage,
                priority,
            )?
        };
        if stage == SetupStage::RoundStart && priority == 2 {
            let settlement = opening_settlement
                .get_or_insert_with(|| begin_round_phase(RoundPhase::RoundStartSettlement));
            append_opening_settlement(settlement, stage_result, version7_opening);
        } else if version7_opening && stage == SetupStage::RoundStart && priority == 1 {
            append_opening_round_phase(
                opening_settlement
                    .as_mut()
                    .expect("Version7 opens settlement before the late setup lane"),
                stage_result,
            );
        } else {
            append(&mut result, stage_result);
        }
        if stage == SetupStage::EnterFight {
            append(
                &mut result,
                run_wave_start_triggers(managers, pool, catalog, determinism, context, 1)?,
            );
        }
        if stage == SetupStage::RoundStart && priority == 2 {
            let (losses, settlement_plan) = run_round_start_loss_mechanics(
                managers,
                pool,
                catalog,
                determinism,
                context,
                None,
            )?;
            let settlement = opening_settlement
                .as_mut()
                .expect("opening round has a settlement phase");
            append_opening_settlement(settlement, losses, version7_opening);
            if settlement_plan.capacity_groups.is_empty() {
                let after_loss = run_round_start_after_loss_mechanics(
                    managers,
                    pool,
                    catalog,
                    determinism,
                    context,
                    &owner_uids,
                    settlement_plan,
                )?;
                append_opening_settlement(settlement, after_loss, version7_opening);
            } else {
                append(
                    &mut result,
                    opening_settlement
                        .take()
                        .expect("opening round has a pre-capacity settlement phase"),
                );
                let mut capacity_sync = begin_round_phase(RoundPhase::RoundStartCapacitySync);
                append_round_phase(
                    &mut capacity_sync,
                    run_round_start_owner_settlement(
                        managers,
                        pool,
                        catalog,
                        determinism,
                        context,
                        settlement_plan,
                    )?,
                );
                append(&mut result, capacity_sync);

                let mut settlement = begin_round_phase(RoundPhase::RoundStartSettlement);
                append_opening_settlement(
                    &mut settlement,
                    run_round_start_skill_mechanics(
                        managers,
                        pool,
                        catalog,
                        determinism,
                        context,
                        &owner_uids,
                    )?,
                    version7_opening,
                );
                opening_settlement = Some(settlement);
            }
        }
        if stage == SetupStage::AfterRoundStart {
            let owner_uids = pool
                .attacker_main
                .iter()
                .filter(|entity| managers.hp.current(entity.uid) > 0)
                .map(|entity| entity.uid)
                .collect::<Vec<_>>();
            append(
                &mut result,
                drain::run_group_event(
                    managers,
                    pool,
                    catalog,
                    determinism,
                    context,
                    BattleEvent::Kind(EventKind::RoundStartCard),
                    drain::ReactionLane::BuffActs,
                    Some(&owner_uids),
                )?,
            );
            append(
                &mut result,
                run_card_energy_allocation(managers, pool, catalog, determinism, context, 1)?,
            );
        }
    }
    let mut opening_refill = super::run_opening_hand_refill(
        managers,
        pool,
        catalog,
        determinism,
        context,
        crate::engine::mechanic::card::CardMechanic.normal_hand_limit(hand_size, managers, pool),
        opening_draws,
    )?;
    let (initial_deck_num, setup_deck_num) =
        opening_deck_counts.expect("start schedule has one CardSetup stage");
    let mut setup_deck_counts = DrainResult::default();
    push_cues(
        &mut setup_deck_counts.frames,
        [
            RoundCue::DeckCount {
                count: initial_deck_num,
                team_type: 1,
            },
            RoundCue::DeckCount {
                count: setup_deck_num,
                team_type: 1,
            },
        ],
    );
    append_round_phase(&mut opening_refill, setup_deck_counts);
    append(&mut result, opening_refill);
    push_cue(
        &mut result.frames,
        RoundCue::DeckCount {
            count: managers.card.deck_num(),
            team_type: 1,
        },
    );
    dealt_cards
        .as_mut()
        .expect("start schedule has one CardSetup stage")
        .extend_from_slice(managers.card.refilled());
    if managers.card.hand_limit_bonus() != 0 {
        append(
            &mut result,
            drain::run(
                managers,
                pool,
                catalog,
                determinism,
                context,
                [RuleOp::Command(BattleCommand::Card(
                    CardCommand::ClearHandLimit {
                        origin: HAND_LIMIT_CLEANUP_ORIGIN,
                    },
                ))],
            )?,
        );
    }
    Ok((
        result,
        dealt_cards.expect("start schedule has one CardSetup stage"),
    ))
}

#[allow(clippy::too_many_arguments)]
fn run_round_start_before_duration(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    team: i32,
    owner_uids: &[i64],
    split_settlement: bool,
) -> Result<(DrainResult, RoundStartSettlementPlan), DrainError> {
    let duration_snapshot = duration_snapshot(managers, owner_uids);
    let field_ops = managers
        .field
        .states()
        .filter(|state| state.team == team)
        .filter_map(|state| {
            let thresholds = crate::engine::skill::behavior::magic_circle::field_thresholds(
                state.definition.field_id,
                state.team,
                managers,
            );
            (!thresholds.is_empty()).then_some(RuleOp::Command(BattleCommand::Field(
                crate::engine::manager::field::FieldCommand {
                    origin: state.origin,
                    team: state.team,
                    operation: crate::engine::manager::field::FieldOperation::ResolveLevel {
                        thresholds,
                    },
                },
            )))
        })
        .collect::<Vec<_>>();
    let mut result = drain::run(managers, pool, catalog, determinism, context, field_ops)?;
    for &(stage, priority) in ROUND_START_BEFORE_DURATION_SETUP {
        append(
            &mut result,
            drain::run_setup_stage_for_owners(
                managers,
                pool,
                catalog,
                determinism,
                context,
                stage,
                priority,
                owner_uids,
            )?,
        );
    }
    append(
        &mut result,
        drain::run_setup_stage_for_owners(
            managers,
            pool,
            catalog,
            determinism,
            context,
            SetupStage::RoundTransitionStart,
            0,
            owner_uids,
        )?,
    );
    let (losses, settlement_plan) = run_round_start_loss_mechanics(
        managers,
        pool,
        catalog,
        determinism,
        context,
        Some(owner_uids),
    )?;
    if split_settlement {
        append(&mut result, losses);
        return Ok((result, settlement_plan));
    }
    let mut round_start_event = begin_round_start_event();
    append_round_phase(&mut round_start_event, losses);
    append_round_phase(
        &mut round_start_event,
        run_round_start_after_loss_mechanics(
            managers,
            pool,
            catalog,
            determinism,
            context,
            owner_uids,
            settlement_plan,
        )?,
    );
    append_round_phase(
        &mut round_start_event,
        drain::run(
            managers,
            pool,
            catalog,
            determinism,
            context,
            duration_advance_rule(effect_time::ROUND_START_DURATION, &duration_snapshot),
        )?,
    );
    append_round_phase(
        &mut round_start_event,
        drain::run_setup_schedule_for_owners_in_round_phase(
            managers,
            pool,
            catalog,
            determinism,
            context,
            ROUND_START_EVENT_SETUP,
            owner_uids,
        )?,
    );
    append(&mut result, round_start_event);
    append(
        &mut result,
        drain::run_setup_schedule_for_owners(
            managers,
            pool,
            catalog,
            determinism,
            context,
            ROUND_START_INDEPENDENT_SETUP,
            owner_uids,
        )?,
    );
    Ok((result, RoundStartSettlementPlan::default()))
}

#[allow(clippy::too_many_arguments)]
fn run_round_start_loss_mechanics(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    buff_owner_uids: Option<&[i64]>,
) -> Result<(DrainResult, RoundStartSettlementPlan), DrainError> {
    let active_features = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .collect::<Vec<_>>();
    let result = drain::run_group_event(
        managers,
        pool,
        catalog,
        determinism,
        context,
        BattleEvent::RoundStart,
        drain::ReactionLane::BuffActsBeforeSettlement,
        buff_owner_uids,
    )?;
    let loss_by_instance = raspberry_losses(&result);
    let capacity_groups = crate::engine::mechanic::shadow_cloak::capacity_rule_groups(
        managers,
        &active_features,
        &loss_by_instance,
    )?;
    let buff_owner_uids = buff_owner_uids.map_or_else(
        || {
            active_features
                .iter()
                .map(|feature| feature.owner_uid)
                .fold(Vec::new(), |mut owners, owner_uid| {
                    if !owners.contains(&owner_uid) {
                        owners.push(owner_uid);
                    }
                    owners
                })
        },
        <[i64]>::to_vec,
    );
    Ok((
        result,
        RoundStartSettlementPlan::new(capacity_groups, buff_owner_uids),
    ))
}

pub(super) fn raspberry_losses(result: &DrainResult) -> std::collections::HashMap<(i64, i64), i32> {
    let mut losses = std::collections::HashMap::new();
    for outcome in &result.outcomes {
        match outcome {
            RuleOutcome::Hp(execution) => record_raspberry_loss(&execution.changes, &mut losses),
            RuleOutcome::HpBatch(batch) => {
                for execution in batch {
                    record_raspberry_loss(&execution.changes, &mut losses);
                }
            }
            _ => {}
        }
    }
    losses
}

fn record_raspberry_loss(
    changes: &crate::engine::manager::hp::HpChanges,
    losses: &mut std::collections::HashMap<(i64, i64), i32>,
) {
    if changes.origin.domain != RuleDomain::BuffAct
        || buff_act::registry::kind(changes.origin.key.opcode, changes.origin.key.type_name)
            != Some(buff_act::registry::BuffActKind::Raspberry)
    {
        return;
    }
    let Some(hp) = changes.hp.filter(|hp| hp.delta < 0) else {
        return;
    };
    let Some(hurt) = hp.hurt.filter(|hurt| hurt.buff_uid != 0) else {
        return;
    };
    let loss = losses.entry((hp.target_uid, hurt.buff_uid)).or_default();
    *loss = loss.saturating_add(hp.delta.saturating_abs());
}

#[allow(clippy::too_many_arguments)]
fn run_round_start_skill_mechanics(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    owner_uids: &[i64],
) -> Result<DrainResult, DrainError> {
    drain::run_group_event(
        managers,
        pool,
        catalog,
        determinism,
        context,
        BattleEvent::RoundStart,
        drain::ReactionLane::Skills,
        Some(owner_uids),
    )
}

fn run_round_start_damage_heal_settlement(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
) -> Result<DrainResult, DrainError> {
    let ops = crate::engine::damage::heal_settlement::take_settlement_rule_ops(managers);
    drain::run(managers, pool, catalog, determinism, context, ops)
}

#[allow(clippy::too_many_arguments)]
fn run_round_start_after_loss_mechanics(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    owner_uids: &[i64],
    settlement_plan: RoundStartSettlementPlan,
) -> Result<DrainResult, DrainError> {
    let mut result = run_round_start_owner_settlement(
        managers,
        pool,
        catalog,
        determinism,
        context,
        settlement_plan,
    )?;
    append(
        &mut result,
        run_round_start_skill_mechanics(managers, pool, catalog, determinism, context, owner_uids)?,
    );
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_round_start_owner_settlement(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    mut settlement_plan: RoundStartSettlementPlan,
) -> Result<DrainResult, DrainError> {
    managers.buff.begin_transaction();
    let owner_settlement: Result<DrainResult, DrainError> = (|| {
        let mut result = DrainResult::default();
        for owner_uid in &settlement_plan.buff_owner_uids {
            if let Some(index) = settlement_plan
                .capacity_groups
                .iter()
                .position(|group| group.owner_uid == *owner_uid)
            {
                append(
                    &mut result,
                    drain::run_command_group(
                        managers,
                        pool,
                        catalog,
                        determinism,
                        context,
                        settlement_plan.capacity_groups.remove(index).operations,
                    )?,
                );
            }
            append(
                &mut result,
                drain::run_group_event(
                    managers,
                    pool,
                    catalog,
                    determinism,
                    context,
                    BattleEvent::RoundStart,
                    drain::ReactionLane::BuffActsAfterSettlement,
                    Some(std::slice::from_ref(owner_uid)),
                )?,
            );
        }
        if !settlement_plan.capacity_groups.is_empty() {
            return Err(
                crate::engine::mechanic::shadow_cloak::CapacityPlanError::UnsettledOwners {
                    owner_uids: settlement_plan
                        .capacity_groups
                        .iter()
                        .map(|group| group.owner_uid)
                        .collect(),
                }
                .into(),
            );
        }
        Ok(result)
    })();
    managers.buff.end_transaction();
    owner_settlement
}

#[allow(clippy::too_many_arguments)]
fn run_round_start_after_duration_setup(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    owner_uids: &[i64],
) -> Result<(DrainResult, DrainResult), DrainError> {
    let event_setup = drain::run_setup_schedule_for_owners_in_round_phase(
        managers,
        pool,
        catalog,
        determinism,
        context,
        ROUND_START_EVENT_SETUP,
        owner_uids,
    )?;
    let independent_setup = drain::run_setup_schedule_for_owners(
        managers,
        pool,
        catalog,
        determinism,
        context,
        ROUND_START_INDEPENDENT_SETUP,
        owner_uids,
    )?;
    Ok((event_setup, independent_setup))
}
