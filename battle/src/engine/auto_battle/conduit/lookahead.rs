use std::collections::HashSet;

use sonettobuf::{BeginRoundOper, FightDeviceOper};

use crate::engine::{
    auto_battle::simulation::Simulation,
    manager::{
        BattleManagers,
        card::{CARD_PLAY_ORIGIN, CardManager, CardPlay},
    },
    runtime::determinism::RoundDeterminism,
};

use super::super::{best_candidate, play_card, play_operation, random_card_choice};

#[allow(clippy::too_many_arguments)]
pub(in crate::engine::auto_battle) fn operations(
    cards: &CardManager,
    simulation: &Simulation<'_>,
    sim: &BattleManagers,
    played_owners: &HashSet<i64>,
    normal_ap: i32,
    preferred_target: Option<i64>,
    determinism: &RoundDeterminism,
    choice_determinism: &RoundDeterminism,
    prefix: &[BeginRoundOper],
    devices: &[FightDeviceOper],
) -> Vec<BeginRoundOper> {
    let mut cards = cards.clone();
    let mut sim = sim.clone();
    let mut sim_determinism = determinism.clone();
    let mut choice_determinism = choice_determinism.clone();
    let mut normal_ap = normal_ap;
    let mut played_owners = played_owners.clone();
    let mut opers = prefix.to_vec();
    let live_pool = simulation.pool.runtime_view(&simulation.managers);
    let selected_power_types = HashSet::new();
    let mut reported_unsupported = HashSet::new();

    loop {
        let choices = cards
            .hand()
            .iter()
            .chain(cards.team_cards())
            .map(|card| random_card_choice(card, &simulation.managers, &mut choice_determinism))
            .collect::<Vec<_>>();
        let Some(candidate) = best_candidate(
            &cards,
            &choices,
            &played_owners,
            normal_ap,
            preferred_target,
            &simulation.managers,
            &sim,
            &live_pool,
            &simulation.pool.runtime_view(&sim),
            simulation.catalog,
            &sim_determinism,
            &selected_power_types,
            &[],
            &mut reported_unsupported,
            simulation,
            &opers,
            devices,
        ) else {
            break;
        };
        let play = CardPlay {
            origin: CARD_PLAY_ORIGIN,
            hand_index: candidate.card_index,
            target_uid: Some(candidate.target_uid),
            chosen_skill_id: candidate.chosen_skill_id,
            choice: None,
            recorded_skill: None,
        };
        opers.push(play_operation(
            candidate.card_index,
            candidate.target_uid,
            candidate.chosen_skill_id,
        ));
        let Some((after, after_determinism)) = simulation.player(&opers, devices) else {
            opers.pop();
            break;
        };
        if !play_card(&mut cards, play) {
            opers.pop();
            break;
        }
        sim = after;
        sim_determinism = after_determinism;
        normal_ap = normal_ap.saturating_sub(candidate.normal_ap);
        if candidate.normal_ap > 0 {
            played_owners.insert(candidate.source_uid);
        }
    }
    opers
}
