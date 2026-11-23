use rand::{SeedableRng, rngs::StdRng};
use sonettobuf::{CardInfo, Fight};

use crate::engine::manager::card::{
    ai::{
        generate_ai_deck_with_extra_actions, generated_ai_action_count, resolve_configured_targets,
    },
    draw::draw_guaranteed_by_uid,
    pool::{
        active_enemy_entities, active_player_uids, card_for, normal_player_candidate_pool_with,
    },
};

const CARDS_PER_HERO: i32 = 16;
const MAX_NORMAL_HAND_SIZE: usize = 8;

pub fn deck_size(fight: &Fight) -> i32 {
    let normal_uids =
        crate::engine::manager::card::pool::normal_player_candidate_pool_with(fight, |_| false)
            .into_iter()
            .filter_map(|card| card.uid)
            .collect::<std::collections::HashSet<_>>();
    i32::try_from(normal_uids.len())
        .unwrap_or(i32::MAX)
        .saturating_mul(CARDS_PER_HERO)
}

pub fn hand_size(fight: &Fight) -> usize {
    hand_size_from_count(
        active_player_uids(fight).len(),
        fight.version.unwrap_or_default(),
    )
}

pub fn hand_size_from_count(characters: usize, fight_version: i32) -> usize {
    match characters {
        0 => 0,
        characters
            if crate::engine::fight::versions::round_start_setup_layout(fight_version)
                == Some(crate::engine::fight::versions::RoundStartSetupLayout::Version7) =>
        {
            (characters * 2 + 1).clamp(4, MAX_NORMAL_HAND_SIZE)
        }
        characters => (characters * 2 + 1).min(MAX_NORMAL_HAND_SIZE),
    }
}

pub(crate) fn preserve_generated_basic_coverage(fight: &Fight, cards: &mut [CardInfo]) {
    if crate::engine::fight::versions::round_start_setup_layout(fight.version.unwrap_or_default())
        != Some(crate::engine::fight::versions::RoundStartSetupLayout::Version7)
    {
        return;
    }
    let basics = normal_player_candidate_pool_with(fight, |_| false);
    preserve_basic_coverage(&basics, cards);
}

pub(crate) fn preserve_basic_coverage(basics: &[CardInfo], cards: &mut [CardInfo]) {
    if basics.len() > cards.len() {
        return;
    }
    for basic in basics {
        if cards.iter().any(|card| same_card(card, basic)) {
            continue;
        }
        if let Some(index) = (0..cards.len()).rev().find(|index| {
            let candidate = &cards[*index];
            !basics.iter().any(|basic| same_card(basic, candidate))
                || cards
                    .iter()
                    .filter(|card| same_card(card, candidate))
                    .count()
                    > 1
        }) {
            cards[index] = basic.clone();
        }
    }
}

fn same_card(left: &CardInfo, right: &CardInfo) -> bool {
    left.uid == right.uid && left.skill_id == right.skill_id
}

pub fn configured_opening_deal(
    game_data: &config::GameDB,
    fight: &Fight,
) -> Result<Option<Vec<CardInfo>>, String> {
    opening_deal_from(fight, |episode_id| {
        crate::catalog::configured_teaching_cards(game_data, episode_id)
    })
}

pub(crate) fn opening_deal(
    catalog: crate::catalog::BattleCatalog,
    fight: &Fight,
) -> Result<Option<Vec<CardInfo>>, String> {
    opening_deal_from(fight, |episode_id| catalog.teaching_cards(episode_id))
}

fn opening_deal_from(
    fight: &Fight,
    configured: impl FnOnce(i32) -> Option<crate::catalog::ConfiguredTeachingCards>,
) -> Result<Option<Vec<CardInfo>>, String> {
    let Some(config) = teaching_card_config(fight, configured) else {
        return Ok(None);
    };
    let cards = resolve_configured_cards(fight, &config.opening_cards)?;
    if cards.is_empty() {
        return Err("teaching-card opening deal is empty".into());
    }
    Ok(Some(cards))
}

pub fn configured_refill_draws(
    game_data: &config::GameDB,
    fight: &Fight,
) -> Result<Vec<CardInfo>, String> {
    refill_draws_from(fight, |episode_id| {
        crate::catalog::configured_teaching_cards(game_data, episode_id)
    })
}

pub(crate) fn refill_draws(
    catalog: crate::catalog::BattleCatalog,
    fight: &Fight,
) -> Result<Vec<CardInfo>, String> {
    refill_draws_from(fight, |episode_id| catalog.teaching_cards(episode_id))
}

fn refill_draws_from(
    fight: &Fight,
    configured: impl FnOnce(i32) -> Option<crate::catalog::ConfiguredTeachingCards>,
) -> Result<Vec<CardInfo>, String> {
    let Some(config) = teaching_card_config(fight, configured) else {
        return Ok(Vec::new());
    };
    resolve_configured_cards(fight, &config.refill_cards)
}

fn teaching_card_config(
    fight: &Fight,
    configured: impl FnOnce(i32) -> Option<crate::catalog::ConfiguredTeachingCards>,
) -> Option<crate::catalog::ConfiguredTeachingCards> {
    if crate::engine::fight::versions::round_start_setup_layout(fight.version.unwrap_or_default())
        != Some(crate::engine::fight::versions::RoundStartSetupLayout::Version7)
    {
        return None;
    }
    configured(fight.episode_id.unwrap_or_default())
}

fn resolve_configured_cards(fight: &Fight, entries: &str) -> Result<Vec<CardInfo>, String> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    let attacker = fight
        .attacker
        .as_ref()
        .ok_or_else(|| "teaching-card battle has no attacker team".to_string())?;
    entries
        .split('|')
        .map(|entry| {
            let mut fields = entry.split('#');
            let model_id = fields
                .next()
                .and_then(|value| value.parse::<i32>().ok())
                .filter(|value| *value > 0)
                .ok_or_else(|| format!("invalid teaching-card owner `{entry}`"))?;
            let group = fields
                .next()
                .and_then(|value| value.parse::<u8>().ok())
                .filter(|value| matches!(value, 1 | 2))
                .ok_or_else(|| format!("invalid teaching-card skill group `{entry}`"))?;
            if fields.next().is_some() {
                return Err(format!("invalid teaching-card entry `{entry}`"));
            }
            let entity = attacker
                .entitys
                .iter()
                .find(|entity| {
                    entity.model_id == Some(model_id) && entity.current_hp.unwrap_or(1) > 0
                })
                .ok_or_else(|| format!("teaching-card owner {model_id} is not in the fight"))?;
            let skill_id = match group {
                1 => entity.skill_group1.first(),
                2 => entity.skill_group2.first(),
                _ => unreachable!(),
            }
            .copied()
            .ok_or_else(|| format!("teaching-card owner {model_id} has no skill group {group}"))?;
            card_for(entity, Some(skill_id))
                .ok_or_else(|| format!("invalid teaching-card skill {skill_id}"))
        })
        .collect()
}

pub fn draw_bag(game_data: &config::GameDB, fight: &Fight) -> Vec<CardInfo> {
    draw_bag_from(fight, |fight| {
        crate::engine::manager::card::pool::device_draw_bag(game_data, fight)
    })
}

pub(crate) fn configured_draw_bag(
    catalog: crate::catalog::BattleCatalog,
    fight: &Fight,
) -> Vec<CardInfo> {
    draw_bag_from(fight, |fight| {
        crate::engine::manager::card::pool::device_draw_bag_from(fight, |entity| {
            catalog.device_card_weights(entity)
        })
    })
}

fn draw_bag_from(
    fight: &Fight,
    device_cards: impl FnOnce(&Fight) -> Vec<CardInfo>,
) -> Vec<CardInfo> {
    let candidates =
        crate::engine::manager::card::pool::normal_player_candidate_pool_with(fight, |_| false);
    let mut cards = active_player_uids(fight)
        .into_iter()
        .flat_map(|uid| {
            let owner = candidates
                .iter()
                .filter(|card| card.uid == Some(uid))
                .cloned()
                .collect::<Vec<_>>();
            (0..CARDS_PER_HERO)
                .filter_map(move |index| owner.get(index as usize % owner.len().max(1)).cloned())
        })
        .collect::<Vec<_>>();
    cards.extend(device_cards(fight));
    cards
}

pub fn start_decks_from_fight(
    game_data: &config::GameDB,
    fight: &Fight,
    ex_point: &crate::engine::manager::ex_point::ExPointManager,
    eureka: &crate::engine::manager::eureka::EurekaManager,
    extra_ai_actions: i32,
    seed_value: i32,
    captured: Option<(Vec<CardInfo>, Vec<CardInfo>)>,
) -> (Vec<CardInfo>, Vec<CardInfo>) {
    let decks = start_decks_from(
        fight,
        ex_point,
        eureka,
        extra_ai_actions,
        seed_value,
        &[],
        captured.map(|(ai, player)| CapturedDeckSeed::Opening {
            ai,
            player,
            reserved_ultimate_slots: 0,
        }),
        |allow_ex_skill| {
            crate::engine::manager::card::pool::player_candidate_pool_from(
                fight,
                |_| allow_ex_skill,
                |entity| crate::catalog::configured_device_card_weights(game_data, entity),
            )
        },
    );
    (decks.ai, decks.player)
}

pub(crate) enum CapturedDeckSeed {
    Opening {
        ai: Vec<CardInfo>,
        player: Vec<CardInfo>,
        reserved_ultimate_slots: usize,
    },
    NextAi(Vec<CardInfo>),
}

pub(crate) struct ConfiguredStartDecks {
    pub ai: Vec<CardInfo>,
    pub player: Vec<CardInfo>,
    pub used_capture: bool,
    used_ai_capture: bool,
}

pub(crate) fn configured_start_decks(
    fight: &Fight,
    managers: &crate::engine::manager::BattleManagers,
    skill_catalog: &crate::engine::skill::effect::SkillEffectCatalog,
    determinism: &crate::engine::runtime::determinism::RoundDeterminism,
    extra_ai_actions: i32,
    seed_value: i32,
    captured: Option<CapturedDeckSeed>,
) -> Result<ConfiguredStartDecks, String> {
    let mut current_fight = fight.clone();
    managers.sync_entities(&mut current_fight);
    let fight = &current_fight;
    let pool = crate::engine::skill::target::TargetPool::from_fight_with_catalog(
        managers.catalog(),
        fight,
    );
    let prepared = super::prepared::cards(&pool, managers.card.prepared_ai_skills())?;
    let mut decks = start_decks_from(
        fight,
        &managers.ex_point,
        &managers.eureka,
        extra_ai_actions,
        seed_value,
        &prepared,
        captured,
        |allow_ex_skill| {
            crate::engine::manager::card::pool::player_candidate_pool_from(
                fight,
                |_| allow_ex_skill,
                |entity| managers.catalog().device_card_weights(entity),
            )
        },
    );
    if !decks.used_ai_capture {
        resolve_configured_targets(fight, managers, skill_catalog, determinism, &mut decks.ai)?;
    }
    Ok(decks)
}

#[allow(clippy::too_many_arguments)]
fn start_decks_from(
    fight: &Fight,
    ex_point: &crate::engine::manager::ex_point::ExPointManager,
    eureka: &crate::engine::manager::eureka::EurekaManager,
    extra_ai_actions: i32,
    seed_value: i32,
    prepared: &[CardInfo],
    captured: Option<CapturedDeckSeed>,
    mut player_candidates: impl FnMut(bool) -> Vec<CardInfo>,
) -> ConfiguredStartDecks {
    let required_uids = active_player_uids(fight);
    let valid_target_uids = fight
        .attacker
        .iter()
        .chain(&fight.defender)
        .flat_map(|team| team.entitys.iter().chain(&team.sp_entitys))
        .filter_map(|entity| entity.uid)
        .collect::<std::collections::HashSet<_>>();
    let hand_size = hand_size(fight);
    let mut rng = StdRng::seed_from_u64(seed(fight, seed_value));
    if let Some(captured) = captured {
        let (captured_ai, captured_player, reserved_ultimate_slots) = match captured {
            CapturedDeckSeed::Opening {
                ai,
                player,
                reserved_ultimate_slots,
            } => (ai, Some(player), reserved_ultimate_slots),
            CapturedDeckSeed::NextAi(ai) => (ai, None, 0),
        };
        let mut captured_candidates = player_candidates(false);
        captured_candidates.extend(player_candidates(true));
        let expected_ai_count =
            generated_ai_action_count(fight, ex_point, eureka, extra_ai_actions) + prepared.len();
        let mut ai_candidates = active_enemy_entities(fight)
            .into_iter()
            .flat_map(|entity| {
                entity
                    .skill_group1
                    .iter()
                    .chain(&entity.skill_group2)
                    .copied()
                    .chain(entity.ex_skill)
                    .filter_map(|skill_id| card_for(entity, Some(skill_id)))
            })
            .collect::<Vec<_>>();
        let mut ordinary_ai = captured_ai.iter().collect::<Vec<_>>();
        let prepared_present = prepared.iter().all(|prepared| {
            let Some(index) = ordinary_ai
                .iter()
                .position(|card| same_card(card, prepared))
            else {
                return false;
            };
            ordinary_ai.remove(index);
            true
        });
        let ordinary_valid = ordinary_ai.iter().all(|card| {
            ai_candidates
                .iter()
                .any(|candidate| same_card(card, candidate))
        });
        ai_candidates.extend_from_slice(prepared);
        let ai_valid = captured_ai.len() == expected_ai_count && prepared_present && ordinary_valid;
        let ai = ai_valid
            .then(|| {
                captured_ai
                    .iter()
                    .map(|captured| {
                        let mut candidate = ai_candidates
                            .iter()
                            .find(|candidate| {
                                captured.uid == candidate.uid
                                    && captured.skill_id == candidate.skill_id
                            })?
                            .clone();
                        // V7 snapshots may retain a defeated roster UID even when skill targeting is source-owned.
                        candidate.target_uid = captured
                            .target_uid
                            .filter(|uid| valid_target_uids.contains(uid))
                            .or(candidate.target_uid);
                        Some(candidate)
                    })
                    .collect::<Option<Vec<_>>>()
            })
            .flatten();
        let player_size_valid = captured_player.as_ref().is_none_or(|player| {
            player.len().checked_add(reserved_ultimate_slots) == Some(hand_size)
        });
        let player = match captured_player.as_ref() {
            None => Some(Vec::new()),
            Some(cards) if player_size_valid => cards
                .iter()
                .map(|captured| {
                    captured_candidates
                        .iter()
                        .find(|candidate| {
                            captured.uid == candidate.uid && captured.skill_id == candidate.skill_id
                        })
                        .cloned()
                })
                .collect::<Option<Vec<_>>>(),
            Some(_) => None,
        };
        if let (Some(ai), Some(player)) = (ai, player.as_ref()) {
            return ConfiguredStartDecks {
                ai,
                player: player.clone(),
                used_capture: true,
                used_ai_capture: true,
            };
        }
        if captured_player.is_some()
            && player_size_valid
            && let Some(player) = player
        {
            let mut ai = generate_ai_deck_with_extra_actions(
                fight,
                ex_point,
                eureka,
                extra_ai_actions,
                &mut rng,
            );
            ai.extend_from_slice(prepared);
            return ConfiguredStartDecks {
                ai,
                player,
                used_capture: true,
                used_ai_capture: false,
            };
        }
    }

    let candidates = player_candidates(false);
    let player = draw_guaranteed_by_uid(&candidates, &required_uids, hand_size, &mut rng);
    let mut ai =
        generate_ai_deck_with_extra_actions(fight, ex_point, eureka, extra_ai_actions, &mut rng);
    ai.extend_from_slice(prepared);
    ConfiguredStartDecks {
        ai,
        player,
        used_capture: false,
        used_ai_capture: false,
    }
}

fn seed(fight: &Fight, seed_value: i32) -> u64 {
    let mut seed = 1_469_598_103_934_665_603_u64 ^ seed_value as u64;
    for uid in active_player_uids(fight) {
        seed = (seed ^ uid as u64).wrapping_mul(1_099_511_628_211);
        if let Some(entity) = fight
            .attacker
            .as_ref()
            .and_then(|team| team.entitys.iter().find(|entity| entity.uid == Some(uid)))
        {
            seed =
                (seed ^ entity.model_id.unwrap_or_default() as u64).wrapping_mul(1_099_511_628_211);
            seed = (seed ^ entity.skill_group1.first().copied().unwrap_or_default() as u64)
                .wrapping_mul(1_099_511_628_211);
            seed = (seed ^ entity.skill_group2.first().copied().unwrap_or_default() as u64)
                .wrapping_mul(1_099_511_628_211);
        }
    }
    seed
}

#[cfg(test)]
mod tests;
