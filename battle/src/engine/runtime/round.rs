use sonettobuf::{BeginRoundRequest, Fight, FightRound, FightStep};

use crate::engine::{
    manager::card::CardCommand,
    round::{
        command::{RoundCommand, commands_from_opers},
        outcome::{battle_ended, finish_if_battle_ended},
        power::ClothPower,
        state::{RoundState, next_action_points, next_round_shell, round_field_cards},
    },
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

use super::{
    BattleRuntime,
    determinism::{self, RoundDeterminism},
    drain, executor, project_result, schedule,
};

impl BattleRuntime {
    pub fn build_player_action_steps(
        &mut self,
        plays: Vec<crate::engine::manager::card::CardPlay>,
        determinism: &mut RoundDeterminism,
        team: i32,
        emitter_uid: i64,
    ) -> Result<Vec<FightStep>, String> {
        let pool = crate::engine::skill::target::TargetPool::from_fight(&self.fight);
        let result = schedule::run_player_action_queue(
            &mut self.managers,
            &pool,
            &self.catalog,
            determinism,
            crate::engine::skill::target::TargetContext::default(),
            plays,
            team,
            emitter_uid,
        )
        .map_err(|error| format!("{error:?}"))?;
        crate::engine::packet::timeline::project_for_version(
            &result.frames,
            self.fight.version.unwrap_or_default(),
        )
        .map_err(|error| format!("{error:?}"))
    }

    pub(super) fn build_begin_round_from_schedule(
        &mut self,
        request: &BeginRoundRequest,
    ) -> Result<FightRound, String> {
        let active_round = self.round_state.cur_round;
        self.round_state.begin_round();
        self.fight.cur_round = Some(self.round_state.cur_round);
        if let Some(attacker) = self.fight.attacker.as_mut() {
            for skill in &mut attacker.skill_infos {
                skill.cd = Some(skill.cd.unwrap_or_default().saturating_sub(1).max(0));
            }
        }
        self.round_state.before_cards2.clear();
        self.round_state.team_a_cards2.clear();
        self.managers.begin_round();
        let current_card_skills = self
            .managers
            .card
            .hand()
            .iter()
            .chain(self.managers.card.draw_pile())
            .chain(self.managers.card.team_cards())
            .filter_map(|card| card.skill_id)
            .filter(|skill_id| *skill_id > 0)
            .chain(self.determinism.card_play_skill_ids())
            .collect::<Vec<_>>();
        self.catalog.extend_roots_and_warn(
            config::configs::get(),
            request
                .opers
                .iter()
                .filter_map(|oper| oper.param3)
                .filter(|skill_id| *skill_id > 0)
                .chain(current_card_skills),
            std::iter::empty(),
        );
        let captured_ai_choices = self.determinism.take_ai_skills();
        if let Some(choices) = &captured_ai_choices {
            self.catalog.extend_roots_and_warn(
                config::configs::get(),
                choices.iter().map(|choice| choice.skill_id),
                std::iter::empty(),
            );
        }
        let mut ai_envelope = self.managers.card.ai_queue().to_vec();

        let catalog = &mut self.catalog;
        let mut pool = crate::engine::skill::target::TargetPool::from_fight(&self.fight);
        let context = crate::engine::skill::target::TargetContext {
            battle_id: self.fight.battle_id.unwrap_or_default(),
            current_round: self.round_state.cur_round,
            ..Default::default()
        };
        let commands = commands_from_opers(&request.opers);
        let fight_version = self.fight.version.unwrap_or_default();
        let uses_action_phase_power_clear =
            crate::engine::fight::versions::round_start_setup_layout(fight_version)
                == Some(crate::engine::fight::versions::RoundStartSetupLayout::Version7);

        let conduit_selection = drain::run(
            &mut self.managers,
            &pool,
            catalog,
            &mut self.determinism,
            context,
            request.devices_opers.iter().filter_map(|operation| {
                Some(crate::engine::skill::rule::output::RuleOp::Command(
                    crate::engine::skill::rule::output::BattleCommand::Conduit(
                        crate::engine::manager::conduit::ConduitCommand::SelectGroup {
                            source_uid: operation.uid?,
                            group: operation.index?,
                        },
                    ),
                ))
            }),
        )
        .map_err(|error| format!("{error:?}"))?;
        let mut fight_steps = project_result(conduit_selection, fight_version)?;
        let player = schedule::run_player_phase(
            &self.fight,
            &mut self.managers,
            &pool,
            catalog,
            &mut self.determinism,
            context,
            commands.iter().cloned(),
            1,
            crate::engine::manager::emitter::UID,
        )
        .map_err(|error| format!("{error:?}"))?;
        let ended_by_player = battle_ended(&self.fight, &pool, &self.managers);
        self.objectives
            .record_player_round(&commands, catalog, &player, ended_by_player);
        apply_cloth_power(&self.fight, &self.managers, &mut self.round_state, &player);
        let conduit_context = selected_enemy_target(&commands, &pool)
            .map(
                |runtime_target_uid| crate::engine::skill::target::TargetContext {
                    runtime_target_uid,
                    ..context
                },
            )
            .unwrap_or(context);
        let conduit = if ended_by_player {
            Default::default()
        } else {
            schedule::run_conduit_phase(
                &self.fight,
                &mut self.managers,
                &pool,
                catalog,
                &mut self.determinism,
                conduit_context,
                &request.devices_opers,
            )
            .map_err(|error| format!("{error:?}"))?
        };
        let hand_size = crate::engine::mechanic::card::CardMechanic.normal_hand_limit(
            crate::engine::manager::card::start::hand_size(&self.fight),
            &self.managers,
            &pool,
        );
        fight_steps.extend(project_result(player, fight_version)?);
        fight_steps.extend(project_result(conduit, fight_version)?);
        let ended_during_attacker_actions = battle_ended(&self.fight, &pool, &self.managers);
        let promotions = if ended_during_attacker_actions {
            Vec::new()
        } else {
            self.managers.promote_reserves(&mut self.fight)
        };
        self.objectives.record_promotions(&promotions);
        if !promotions.is_empty() {
            self.managers.sync_roster(&self.fight);
            pool = crate::engine::skill::target::TargetPool::from_fight(&self.fight);
        }
        if !promotions.is_empty() {
            fight_steps.extend(project_result(
                schedule::run_promotions(
                    &self.fight,
                    &mut self.managers,
                    catalog,
                    &mut self.determinism,
                    context,
                    promotions,
                )
                .map_err(|error| format!("{error:?}"))?,
                fight_version,
            )?);
            ai_envelope = self.managers.card.ai_queue().to_vec();
        }
        let attacker_settlement = if ended_during_attacker_actions {
            schedule::run_finished_attacker_settlement(
                &mut self.managers,
                &pool,
                catalog,
                &mut self.determinism,
                context,
            )
        } else {
            schedule::run_attacker_round_end(
                &mut self.managers,
                &pool,
                catalog,
                &mut self.determinism,
                context,
            )
        }
        .map_err(|error| format!("{error:?}"))?;
        fight_steps.extend(project_result(attacker_settlement, fight_version)?);
        let ended_after_attacker_settlement = battle_ended(&self.fight, &pool, &self.managers);
        let current_wave_defeated =
            crate::engine::round::outcome::defenders_defeated(&pool, &self.managers);
        let runs_enemy_phase = !ended_after_attacker_settlement && !current_wave_defeated;
        let needs_refill = crate::engine::mechanic::card::CardMechanic
            .refill_hand_len(&self.managers, &pool)
            < hand_size;
        if needs_refill && runs_enemy_phase {
            self.round_state.before_cards2 = round_field_cards(self.managers.card.hand());
            fight_steps.extend(project_result(schedule::run_round_deal(2), fight_version)?);
        }
        if !ended_after_attacker_settlement {
            let defeated_defenders = pool
                .defender_all
                .iter()
                .filter(|entity| self.managers.hp.current(entity.uid) <= 0)
                .map(|entity| entity.uid)
                .collect::<Vec<_>>();
            fight_steps.extend(project_result(
                schedule::run_entity_card_cleanup(
                    &mut self.managers,
                    &pool,
                    catalog,
                    &mut self.determinism,
                    context,
                    defeated_defenders,
                )
                .map_err(|error| format!("{error:?}"))?,
                fight_version,
            )?);
        }
        if needs_refill && runs_enemy_phase {
            let refill = schedule::run_round_refill(
                &mut self.managers,
                &pool,
                catalog,
                &mut self.determinism,
                context,
                hand_size,
                1,
            )
            .map_err(|error| format!("{error:?}"))?;
            apply_cloth_power(&self.fight, &self.managers, &mut self.round_state, &refill);
            fight_steps.extend(project_result(refill, fight_version)?);
            self.round_state.team_a_cards2 = round_field_cards(self.managers.card.refilled());
        }
        if runs_enemy_phase {
            if uses_action_phase_power_clear {
                fight_steps.extend(project_result(
                    schedule::run_action_phase_start(
                        &mut self.managers,
                        &pool,
                        catalog,
                        &mut self.determinism,
                        context,
                        2,
                    )
                    .map_err(|error| format!("{error:?}"))?,
                    fight_version,
                )?);
            }
            let wave_entry_condition_uids = std::mem::take(&mut self.wave_entry_condition_uids);
            fight_steps.extend(project_result(
                schedule::run_before_ai_round_start(
                    &mut self.managers,
                    &pool,
                    catalog,
                    &mut self.determinism,
                    context,
                    1,
                    &wave_entry_condition_uids,
                )
                .map_err(|error| format!("{error:?}"))?,
                fight_version,
            )?);
            let choices = captured_ai_choices.unwrap_or_else(|| {
                ai_envelope
                    .iter()
                    .filter_map(|card| {
                        Some(determinism::AiSkillChoice {
                            source_uid: card.uid?,
                            skill_id: card.skill_id?,
                            target_uid: card.target_uid.unwrap_or_default(),
                        })
                    })
                    .collect()
            });
            let ai = schedule::run_ai_actions(
                &self.fight,
                &mut self.managers,
                &pool,
                catalog,
                &mut self.determinism,
                context,
                choices,
            )
            .map_err(|error| format!("{error:?}"))?;
            fight_steps.extend(project_result(ai, fight_version)?);
            let after_ai_round_end = schedule::run_after_ai_round_end(
                &mut self.managers,
                &pool,
                catalog,
                &mut self.determinism,
                context,
            )
            .map_err(|error| format!("{error:?}"))?;
            fight_steps.extend(project_result(after_ai_round_end, fight_version)?);
        }
        let mut wave_entering_uids = Vec::new();
        if current_wave_defeated {
            sync_attacker_team_state(
                &mut self.fight,
                self.managers.card.deck_num(),
                self.round_state.power,
            );
        }
        if current_wave_defeated
            && let Some(change) = self
                .managers
                .advance_wave(&mut self.fight)
                .map_err(|error| error.to_string())?
        {
            wave_entering_uids = change.entering_uids.clone();
            catalog.extend_entities_and_warn(
                config::configs::get(),
                crate::engine::manager::wave::entering_entities(&change),
            );
            pool = crate::engine::skill::target::TargetPool::from_fight(&self.fight);
            fight_steps.extend(project_result(
                schedule::run_wave_entry(
                    &mut self.managers,
                    &pool,
                    catalog,
                    &mut self.determinism,
                    context,
                    change,
                )
                .map_err(|error| format!("{error:?}"))?,
                fight_version,
            )?);
            let (next_ai, _) = crate::engine::manager::card::start_decks_from_fight(
                &self.fight,
                &self.managers.ex_point,
                &self.managers.eureka,
                self.fight.battle_id.unwrap_or_default(),
                None,
            );
            catalog.extend_roots_and_warn(
                config::configs::get(),
                next_ai.iter().filter_map(|card| card.skill_id),
                std::iter::empty(),
            );
            self.managers
                .execute_card(CardCommand::SetAiQueue(
                    crate::engine::manager::card::CardSetAiQueue {
                        origin: CommandOrigin {
                            domain: RuleDomain::Lifecycle,
                            key: DefinitionKey::new(0, "WaveAiQueue"),
                        },
                        cards: next_ai,
                    },
                ))
                .map_err(|error| format!("{error:?}"))?;
        }

        if finish_if_battle_ended(&mut self.round_state, &self.fight, &pool, &self.managers) {
            self.round_state.act_point = 0;
        }
        let (
            round_start,
            next_round,
            next_round_hand_snapshot,
            next_round_dealt_cards,
            next_round_prepared,
        ) = if self.round_state.is_finish {
            if uses_action_phase_power_clear {
                let (_, next_round) = schedule::run_finished_round_transition(&self.managers);
                (
                    Default::default(),
                    next_round,
                    Vec::new(),
                    Vec::new(),
                    false,
                )
            } else {
                let (round_start, next_round) =
                    schedule::run_finished_round_transition(&self.managers);
                (round_start, next_round, Vec::new(), Vec::new(), false)
            }
        } else {
            // Snapshot next-round AP before lifecycle cleanup removes one-round
            // modifiers that have already contributed to this value.
            self.round_state.act_point = next_action_points(
                &self.fight,
                &pool,
                &self.managers,
                catalog,
                &mut self.determinism,
                context,
            );
            if let Some(power) = ClothPower::for_fight(&self.fight) {
                self.round_state.power =
                    power.recover_round(self.round_state.power, self.round_state.cur_round);
            }
            let (round_start, next_round, hand_snapshot, dealt_cards) =
                schedule::run_round_start_after_ai_split(
                    &mut self.managers,
                    &pool,
                    catalog,
                    &mut self.determinism,
                    context,
                    &wave_entering_uids,
                    hand_size,
                )
                .map_err(|error| format!("{error:?}"))?;
            if !wave_entering_uids.is_empty() {
                self.wave_entry_condition_uids = wave_entering_uids;
            }
            (round_start, next_round, hand_snapshot, dealt_cards, true)
        };
        finish_if_battle_ended(&mut self.round_state, &self.fight, &pool, &self.managers);
        fight_steps.extend(project_result(round_start, fight_version)?);
        if !self.round_state.is_finish {
            let cards = crate::engine::manager::card::start_decks_from_fight(
                &self.fight,
                &self.managers.ex_point,
                &self.managers.eureka,
                self.round_state.cur_round,
                self.determinism
                    .take_next_ai_card_snapshot()
                    .map(|cards| (cards, Vec::new())),
            )
            .0;
            catalog.extend_roots_and_warn(
                config::configs::get(),
                cards.iter().filter_map(|card| card.skill_id),
                std::iter::empty(),
            );
            fight_steps.extend(project_result(
                schedule::run_ai_queue_refresh(
                    &mut self.managers,
                    &pool,
                    catalog,
                    &mut self.determinism,
                    context,
                    cards,
                )
                .map_err(|error| format!("{error:?}"))?,
                fight_version,
            )?);
        }
        let next_round_begin_step = project_result(next_round, fight_version)?;

        self.managers.sync_entities(&mut self.fight);
        if uses_action_phase_power_clear && self.round_state.is_finish {
            self.round_state.cur_round = active_round;
        }
        self.round_state.hero_sp_attributes = self.managers.hero_sp_attributes(&self.fight);
        self.round_state.last_change_hero_uid = self.fight.last_change_hero_uid;
        self.fight.cur_round = Some(self.round_state.cur_round);
        self.fight.is_finish = Some(self.round_state.is_finish);
        let mut round = next_round_shell(
            &self.fight,
            &self.round_state,
            next_round_prepared,
            &next_round_hand_snapshot,
            &next_round_dealt_cards,
            self.managers.card.ai_queue(),
        );
        if self.round_state.is_finish {
            self.managers
                .execute_card(CardCommand::SetTeamCards(
                    crate::engine::manager::card::CardSetTeamCards {
                        origin: CommandOrigin {
                            domain: RuleDomain::Lifecycle,
                            key: DefinitionKey::new(0, "BattleFinishTeamCards"),
                        },
                        cards: Vec::new(),
                    },
                ))
                .map_err(|error| format!("{error:?}"))?;
        }
        round.fight_step = fight_steps;
        round.next_round_begin_step = next_round_begin_step;
        round.ex_point_info = self.managers.ex_point_info(&self.fight);
        Ok(round)
    }

    pub fn build_begin_round_with_determinism(
        &mut self,
        request: &BeginRoundRequest,
        determinism: RoundDeterminism,
    ) -> Result<FightRound, String> {
        self.determinism = determinism;
        self.build_begin_round_from_schedule(request)
    }

    pub fn advance_round(&mut self, request: BeginRoundRequest) -> Result<FightRound, String> {
        let round = self.build_begin_round_from_schedule(&request)?;
        self.round = Some(round.clone());
        Ok(round)
    }
}

pub(super) fn selected_enemy_target(
    commands: &[RoundCommand],
    pool: &crate::engine::skill::target::TargetPool,
) -> Option<i64> {
    commands.iter().rev().find_map(|command| match command {
        RoundCommand::PlayCard {
            target_uid: Some(target_uid),
            ..
        }
        | RoundCommand::UseAssistBoss {
            target_uid: Some(target_uid),
            ..
        } if pool.team_type(*target_uid) == Some(2) => Some(*target_uid),
        _ => None,
    })
}

fn apply_cloth_power(
    fight: &Fight,
    managers: &crate::engine::manager::BattleManagers,
    state: &mut RoundState,
    result: &drain::DrainResult,
) {
    let Some(power) = ClothPower::for_fight(fight) else {
        return;
    };
    let mut moved_card = false;
    for outcome in &result.outcomes {
        let executor::RuleOutcome::Card(changes) = outcome else {
            continue;
        };
        let composition_from_move =
            changes.kind == crate::engine::manager::card::CardChangeKind::Composed && moved_card;
        state.power = cloth_power_after_card_change(
            &power,
            state.power,
            changes.kind,
            changes.played.is_some(),
            eligible_composition_count(managers, &changes.composed_owners),
            composition_from_move,
        );
        moved_card = changes.kind == crate::engine::manager::card::CardChangeKind::Moved;
    }
}

pub(super) fn eligible_composition_count(
    managers: &crate::engine::manager::BattleManagers,
    owners: &[i64],
) -> usize {
    owners
        .iter()
        .filter(|owner_uid| managers.ex_point.gains_composition_ex_point(**owner_uid))
        .count()
}

pub(super) fn cloth_power_after_card_change(
    power: &ClothPower,
    current: i32,
    kind: crate::engine::manager::card::CardChangeKind,
    has_played: bool,
    composed_count: usize,
    composition_from_move: bool,
) -> i32 {
    match kind {
        crate::engine::manager::card::CardChangeKind::Moved => power.card_moved(current),
        crate::engine::manager::card::CardChangeKind::Played if has_played => {
            power.card_used(current)
        }
        crate::engine::manager::card::CardChangeKind::Refilled if composed_count > 0 => {
            power.cards_composed(current, composed_count)
        }
        crate::engine::manager::card::CardChangeKind::Composed if composition_from_move => {
            power.cards_composed(current, composed_count)
        }
        _ => current,
    }
}

fn sync_attacker_team_state(fight: &mut Fight, deck_num: i32, power: i32) {
    let Some(attacker) = fight.attacker.as_mut() else {
        return;
    };
    attacker.card_deck_size = Some(deck_num);
    attacker.power = Some(power);
}
