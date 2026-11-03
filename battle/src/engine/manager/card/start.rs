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
    if !decks.used_capture {
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
        if captured_ai.len() == expected_ai_count
            && prepared_present
            && ordinary_valid
            && captured_player.as_ref().is_none_or(|player| {
                player.len().checked_add(reserved_ultimate_slots) == Some(hand_size)
            })
        {
            let ai = captured_ai
                .iter()
                .map(|captured| {
                    let mut candidate = ai_candidates
                        .iter()
                        .find(|candidate| {
                            captured.uid == candidate.uid && captured.skill_id == candidate.skill_id
                        })?
                        .clone();
                    // V7 snapshots may retain a defeated roster UID even when skill targeting is source-owned.
                    candidate.target_uid = captured
                        .target_uid
                        .filter(|uid| valid_target_uids.contains(uid))
                        .or(candidate.target_uid);
                    Some(candidate)
                })
                .collect::<Option<Vec<_>>>();
            let player = captured_player
                .as_ref()
                .map(|cards| {
                    cards
                        .iter()
                        .map(|captured| {
                            captured_candidates
                                .iter()
                                .find(|candidate| {
                                    captured.uid == candidate.uid
                                        && captured.skill_id == candidate.skill_id
                                })
                                .cloned()
                        })
                        .collect::<Option<Vec<_>>>()
                })
                .unwrap_or_else(|| Some(Vec::new()));
            if let (Some(ai), Some(player)) = (ai, player) {
                return ConfiguredStartDecks {
                    ai,
                    player,
                    used_capture: true,
                };
            }
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
mod tests {
    use sonettobuf::{Fight, FightEntityInfo, FightTeam};

    use super::*;

    #[test]
    fn builds_start_decks_from_fight_entities() {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![
                    entity(10, 1001, 2, &[101], &[201]),
                    entity(11, 1002, 1, &[102], &[202]),
                ],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![entity(-1, 2001, 1, &[301], &[401])],
                ..Default::default()
            }),
            ..Default::default()
        };

        let mut ex_point = crate::engine::manager::ex_point::ExPointManager::default();
        ex_point.seed(&fight);
        let mut eureka = crate::engine::manager::eureka::EurekaManager::default();
        eureka.seed(&fight);
        let (ai, player) = start_decks_from_fight(
            crate::test_support::game_data(),
            &fight,
            &ex_point,
            &eureka,
            0,
            7,
            None,
        );
        let managers = crate::engine::manager::BattleManagers::seeded(&fight);
        let mut skill_catalog = crate::engine::skill::effect::SkillEffectCatalog::default();
        skill_catalog.insert(crate::engine::skill::effect::slot::ParsedSkillEffect {
            skill_id: 301,
            slots: Vec::new(),
        });
        skill_catalog.insert_logic_target(301, 201);
        let configured = configured_start_decks(
            &fight,
            &managers,
            &skill_catalog,
            &crate::engine::runtime::determinism::RoundDeterminism::default(),
            0,
            7,
            None,
        )
        .unwrap();
        assert_eq!(
            (configured.ai, configured.player),
            (ai.clone(), player.clone())
        );
        assert!(!configured.used_capture);

        assert_eq!(player.len(), 5);
        assert!(player.iter().any(|card| card.uid == Some(10)));
        assert!(player.iter().any(|card| card.uid == Some(11)));
        assert_eq!(ai.len(), 1);
        assert_eq!(ai[0].uid, Some(-1));
        assert_eq!(ai[0].skill_id, Some(301));
        assert!(matches!(ai[0].target_uid, Some(10 | 11)));
    }

    #[test]
    fn generated_basic_coverage_is_versioned_and_does_not_require_device_owners() {
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![
                    entity(10, 1001, 1, &[101], &[201]),
                    entity(11, 1002, 2, &[102], &[202]),
                    entity(12, 1003, 3, &[103], &[203]),
                    entity(13, 1004, 4, &[], &[]),
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        let card = |uid, skill_id| {
            card_for(
                fight
                    .attacker
                    .as_ref()
                    .unwrap()
                    .entitys
                    .iter()
                    .find(|entity| entity.uid == Some(uid))
                    .unwrap(),
                Some(skill_id),
            )
            .unwrap()
        };
        let original = vec![
            card(10, 101),
            card(11, 102),
            card(12, 103),
            card(10, 201),
            card(11, 202),
            card(13, 301),
            card(13, 301),
            card(13, 301),
        ];
        let mut covered = original.clone();
        preserve_generated_basic_coverage(&fight, &mut covered);

        for skill_id in [101, 201, 102, 202, 103, 203] {
            assert!(covered.iter().any(|card| card.skill_id == Some(skill_id)));
        }
        assert_eq!(
            covered.iter().filter(|card| card.uid == Some(13)).count(),
            2
        );

        let mut legacy = original.clone();
        let mut legacy_fight = fight.clone();
        legacy_fight.version = Some(6);
        preserve_generated_basic_coverage(&legacy_fight, &mut legacy);
        assert_eq!(legacy, original);

        let mut devices = vec![card(13, 301), card(13, 301)];
        let device_fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![entity(13, 1004, 1, &[], &[])],
                ..Default::default()
            }),
            ..Default::default()
        };
        preserve_generated_basic_coverage(&device_fight, &mut devices);
        assert_eq!(devices, vec![card(13, 301), card(13, 301)]);
    }

    #[test]
    fn version_seven_normal_hand_has_a_four_card_minimum_and_eight_card_cap() {
        let fight = |characters, version| Fight {
            version: Some(version),
            attacker: Some(FightTeam {
                entitys: (0..characters)
                    .map(|index| {
                        entity(
                            index + 1,
                            1000 + index as i32,
                            index as i32 + 1,
                            &[101],
                            &[201],
                        )
                    })
                    .collect(),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_eq!(hand_size(&fight(0, 7)), 0);
        assert_eq!(hand_size(&fight(1, 6)), 3);
        assert_eq!(hand_size(&fight(1, 7)), 4);
        assert_eq!(hand_size(&fight(2, 7)), 5);
        assert_eq!(hand_size(&fight(3, 7)), 7);
        assert_eq!(hand_size(&fight(4, 7)), 8);
    }

    #[test]
    fn captured_start_decks_accept_two_configured_ai_cards_for_one_enemy() {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![entity(12, 1002, 1, &[202], &[203])],
                sp_entitys: vec![entity(13, 3002, 1, &[], &[])],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![entity(-2, 2002, 1, &[302], &[303])],
                ..Default::default()
            }),
            ..Default::default()
        };
        let captured_player = CardInfo {
            uid: Some(12),
            skill_id: Some(202),
            card_effect: Some(999),
            ..Default::default()
        };
        let captured = (
            vec![
                CardInfo {
                    uid: Some(-2),
                    skill_id: Some(302),
                    card_effect: Some(999),
                    target_uid: Some(13),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(-2),
                    skill_id: Some(303),
                    target_uid: Some(999),
                    ..Default::default()
                },
            ],
            vec![captured_player; hand_size(&fight)],
        );

        let mut ex_point = crate::engine::manager::ex_point::ExPointManager::default();
        ex_point.seed(&fight);
        let mut eureka = crate::engine::manager::eureka::EurekaManager::default();
        eureka.seed(&fight);
        let (ai, player) = start_decks_from_fight(
            crate::test_support::game_data(),
            &fight,
            &ex_point,
            &eureka,
            1,
            0,
            Some(captured),
        );

        assert_eq!(ai[0].skill_id, Some(302));
        assert_eq!(ai[0].card_effect, None);
        assert_eq!(ai[0].target_uid, Some(13));
        assert_eq!(ai[1].skill_id, Some(303));
        assert_eq!(ai[1].target_uid, Some(0));
        assert_eq!(ai.len(), 2);
        assert_eq!(player[0].skill_id, Some(202));
        assert_eq!(player[0].card_effect, None);
        assert_eq!(player.len(), hand_size(&fight));
    }

    #[test]
    fn captured_opening_reserves_normal_ultimate_slots_without_padding_the_seed() {
        let mut attacker = entity(12, 1002, 1, &[202], &[203]);
        attacker.ex_skill = Some(900);
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![attacker],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![entity(-2, 2002, 1, &[302], &[303])],
                ..Default::default()
            }),
            ..Default::default()
        };
        let player_card = CardInfo {
            uid: Some(12),
            skill_id: Some(202),
            ..Default::default()
        };

        let configured = configured_start_decks(
            &fight,
            &crate::engine::manager::BattleManagers::seeded(&fight),
            &crate::engine::skill::effect::SkillEffectCatalog::default(),
            &crate::engine::runtime::determinism::RoundDeterminism::default(),
            0,
            0,
            Some(CapturedDeckSeed::Opening {
                ai: vec![CardInfo {
                    uid: Some(-2),
                    skill_id: Some(302),
                    ..Default::default()
                }],
                player: vec![player_card; hand_size(&fight) - 1],
                reserved_ultimate_slots: 1,
            }),
        )
        .unwrap();

        assert!(configured.used_capture);
        assert_eq!(configured.player.len(), hand_size(&fight) - 1);
        assert!(
            configured
                .player
                .iter()
                .all(|card| card.skill_id == Some(202))
        );
    }

    #[test]
    fn captured_next_ai_snapshot_validates_without_a_player_hand() {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![entity(12, 1002, 1, &[202], &[203])],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![entity(-2, 2002, 1, &[302], &[303])],
                ..Default::default()
            }),
            ..Default::default()
        };
        let configured = configured_start_decks(
            &fight,
            &crate::engine::manager::BattleManagers::seeded(&fight),
            &crate::engine::skill::effect::SkillEffectCatalog::default(),
            &crate::engine::runtime::determinism::RoundDeterminism::default(),
            1,
            0,
            Some(CapturedDeckSeed::NextAi(vec![
                CardInfo {
                    uid: Some(-2),
                    skill_id: Some(302),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(-2),
                    skill_id: Some(303),
                    ..Default::default()
                },
            ])),
        )
        .unwrap();

        assert_eq!(
            configured
                .ai
                .iter()
                .filter_map(|card| card.skill_id)
                .collect::<Vec<_>>(),
            vec![302, 303]
        );
        assert!(configured.player.is_empty());
        assert!(configured.used_capture);
    }

    #[test]
    fn captured_next_ai_snapshot_rejects_a_replaced_target() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![entity(10, 1002, 1, &[202], &[203])],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![
                    entity(-1, 116330001, 1, &[116330021], &[]),
                    FightEntityInfo {
                        current_hp: Some(0),
                        ..entity(-2, 116330100, 2, &[116330112], &[])
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);
        managers
            .execute_entity(crate::engine::manager::entity::EntityCommand {
                origin: crate::engine::skill::rule::CommandOrigin {
                    domain: crate::engine::skill::rule::RuleDomain::Behavior,
                    key: crate::engine::skill::rule::DefinitionKey::new(60008, "Summon"),
                },
                source_uid: -1,
                target_uid: -1,
                operation: crate::engine::manager::entity::EntityOperation::SummonCombatant {
                    model_id: 116330100,
                    position: 2,
                },
            })
            .unwrap();

        let configured = configured_start_decks(
            &fight,
            &managers,
            &crate::engine::skill::effect::SkillEffectCatalog::default(),
            &crate::engine::runtime::determinism::RoundDeterminism::default(),
            0,
            0,
            Some(CapturedDeckSeed::NextAi(vec![
                CardInfo {
                    uid: Some(-1),
                    skill_id: Some(116330021),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(-3),
                    skill_id: Some(116330112),
                    target_uid: Some(-2),
                    ..Default::default()
                },
            ])),
        )
        .unwrap();

        assert!(configured.used_capture);
        assert_eq!(
            configured
                .ai
                .iter()
                .filter_map(|card| card.uid)
                .collect::<Vec<_>>(),
            vec![-1, -3]
        );
        assert_eq!(configured.ai[1].target_uid, Some(0));
    }

    #[test]
    fn captured_start_decks_reject_count_or_identity_mismatch_as_a_whole() {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![entity(12, 1002, 1, &[202], &[203])],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![entity(-2, 2002, 1, &[302], &[303])],
                ..Default::default()
            }),
            ..Default::default()
        };
        let ai_card = |skill_id| CardInfo {
            uid: Some(-2),
            skill_id: Some(skill_id),
            ..Default::default()
        };
        let player_card = || CardInfo {
            uid: Some(12),
            skill_id: Some(202),
            ..Default::default()
        };
        let valid_player = || vec![player_card(); hand_size(&fight)];
        let normal = {
            let mut ex_point = crate::engine::manager::ex_point::ExPointManager::default();
            ex_point.seed(&fight);
            let mut eureka = crate::engine::manager::eureka::EurekaManager::default();
            eureka.seed(&fight);
            start_decks_from_fight(
                crate::test_support::game_data(),
                &fight,
                &ex_point,
                &eureka,
                1,
                0,
                None,
            )
        };

        for captured in [
            (vec![ai_card(302)], valid_player()),
            (
                vec![ai_card(302), ai_card(303), ai_card(302)],
                valid_player(),
            ),
            (vec![ai_card(302), ai_card(999)], valid_player()),
            (vec![ai_card(302), ai_card(303)], vec![player_card()]),
            (
                vec![ai_card(302), ai_card(303)],
                vec![
                    player_card(),
                    CardInfo {
                        uid: Some(12),
                        skill_id: Some(999),
                        ..Default::default()
                    },
                    player_card(),
                ],
            ),
        ] {
            let mut ex_point = crate::engine::manager::ex_point::ExPointManager::default();
            ex_point.seed(&fight);
            let mut eureka = crate::engine::manager::eureka::EurekaManager::default();
            eureka.seed(&fight);
            assert_eq!(
                start_decks_from_fight(
                    crate::test_support::game_data(),
                    &fight,
                    &ex_point,
                    &eureka,
                    1,
                    0,
                    Some(captured),
                ),
                normal
            );
        }
    }

    #[test]
    fn draw_bag_keeps_sixteen_balanced_rank_one_cards_per_hero() {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![entity(10, 1001, 1, &[101], &[201])],
                ..Default::default()
            }),
            ..Default::default()
        };

        let bag = draw_bag(crate::test_support::game_data(), &fight);

        assert_eq!(bag.len(), 16);
        assert_eq!(
            bag.iter().filter(|card| card.skill_id == Some(101)).count(),
            8
        );
        assert_eq!(
            bag.iter().filter(|card| card.skill_id == Some(201)).count(),
            8
        );
    }

    #[test]
    fn configured_opening_deals_do_not_change_version_six_replays() {
        crate::test_support::init_config();
        let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
        let fight = Fight {
            episode_id: Some(10002),
            version: Some(6),
            attacker: Some(FightTeam {
                entitys: vec![
                    entity(-1, 100102, 1, &[30250111], &[30250121]),
                    entity(-2, 100101, 2, &[30230111], &[30230121]),
                ],
                ..Default::default()
            }),
            ..Default::default()
        };

        assert!(
            configured_opening_deal(crate::test_support::game_data(), &fight)
                .unwrap()
                .is_none()
        );
        assert!(
            configured_refill_draws(crate::test_support::game_data(), &fight)
                .unwrap()
                .is_empty()
        );
        assert!(opening_deal(catalog, &fight).unwrap().is_none());
        assert!(refill_draws(catalog, &fight).unwrap().is_empty());
    }

    #[test]
    fn configured_device_cards_extend_the_draw_bag_without_inflating_the_normal_deck() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![entity(10, 3149, 1, &[31490111], &[31490131])],
                ..Default::default()
            }),
            ..Default::default()
        };

        let bag = draw_bag(crate::test_support::game_data(), &fight);
        let configured = configured_draw_bag(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            &fight,
        );

        assert_eq!(configured, bag);

        assert_eq!(deck_size(&fight), 16);
        assert_eq!(bag.len(), 26);
        assert_eq!(
            bag.iter()
                .filter(|card| card.skill_id == Some(31446011))
                .count(),
            2
        );
        assert_eq!(
            bag.iter()
                .filter(|card| card.skill_id == Some(31490201))
                .count(),
            1
        );
    }

    fn entity(
        uid: i64,
        model_id: i32,
        position: i32,
        skill_group1: &[i32],
        skill_group2: &[i32],
    ) -> FightEntityInfo {
        FightEntityInfo {
            uid: Some(uid),
            model_id: Some(model_id),
            position: Some(position),
            current_hp: Some(100),
            skill_group1: skill_group1.to_vec(),
            skill_group2: skill_group2.to_vec(),
            ..Default::default()
        }
    }
}
