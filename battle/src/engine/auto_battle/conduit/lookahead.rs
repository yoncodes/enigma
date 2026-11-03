use std::collections::HashSet;

use crate::engine::{
    manager::{
        BattleManagers,
        card::{CARD_PLAY_ORIGIN, CardManager, CardPlay},
    },
    runtime::determinism::RoundDeterminism,
    skill::{effect::SkillEffectCatalog, target::TargetPool},
};

use super::super::{best_candidate, play_card, random_card_choice, simulate_play};

#[allow(clippy::too_many_arguments)]
pub(in crate::engine::auto_battle) fn state(
    cards: &CardManager,
    managers: &BattleManagers,
    sim: &BattleManagers,
    played_owners: &HashSet<i64>,
    normal_ap: i32,
    preferred_target: Option<i64>,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
) -> (BattleManagers, RoundDeterminism) {
    let mut cards = cards.clone();
    let mut sim = sim.clone();
    let mut sim_determinism = determinism.clone();
    let mut normal_ap = normal_ap;
    let mut played_owners = played_owners.clone();
    let live_pool = pool.runtime_view(managers);
    let selected_power_types = HashSet::new();
    let mut reported_unsupported = HashSet::new();

    loop {
        let choices = cards
            .hand()
            .iter()
            .chain(cards.team_cards())
            .map(|card| random_card_choice(card, managers, &mut sim_determinism))
            .collect::<Vec<_>>();
        let Some(candidate) = best_candidate(
            &cards,
            &choices,
            &played_owners,
            normal_ap,
            preferred_target,
            managers,
            &sim,
            &live_pool,
            &pool.runtime_view(&sim),
            catalog,
            &sim_determinism,
            &selected_power_types,
            &[],
            &mut reported_unsupported,
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
        if !simulate_play(
            &cards,
            &mut sim,
            pool,
            catalog,
            &mut sim_determinism,
            play.clone(),
        ) || !play_card(&mut cards, play)
        {
            break;
        }
        normal_ap = normal_ap.saturating_sub(candidate.normal_ap);
        if candidate.normal_ap > 0 {
            played_owners.insert(candidate.source_uid);
        }
    }
    (sim, sim_determinism)
}
