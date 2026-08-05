use rand::Rng;
use sonettobuf::{CardInfo, Fight};

use crate::engine::manager::{
    eureka::{EurekaManager, PowerType},
    ex_point::ExPointManager,
};

use super::pool::{active_enemy_entities, active_player_uids, card_for};

pub fn generate_ai_deck<R: Rng + ?Sized>(
    fight: &Fight,
    ex_point: &ExPointManager,
    eureka: &EurekaManager,
    rng: &mut R,
) -> Vec<CardInfo> {
    generate_ai_deck_with_extra_actions(fight, ex_point, eureka, 0, rng)
}

pub fn generate_ai_deck_with_extra_actions<R: Rng + ?Sized>(
    fight: &Fight,
    ex_point: &ExPointManager,
    eureka: &EurekaManager,
    extra_actions: i32,
    rng: &mut R,
) -> Vec<CardInfo> {
    let enemies = active_enemy_entities(fight);
    if enemies.is_empty() {
        return Vec::new();
    }

    let target_uids = active_player_uids(fight);
    if target_uids.is_empty() {
        return Vec::new();
    }
    let mut cards = enemies
        .iter()
        .filter_map(|entity| {
            let mut card = card_for(entity, select_skill(entity, ex_point, eureka))?;
            card.target_uid = target_uids
                .get(rng.random_range(0..target_uids.len()))
                .copied();
            Some(card)
        })
        .collect::<Vec<_>>();
    let candidates = cards.clone();
    let action_count = i32::try_from(cards.len())
        .unwrap_or(i32::MAX)
        .saturating_add(extra_actions)
        .max(0) as usize;
    cards.truncate(action_count);
    if candidates.is_empty() {
        return cards;
    }
    while cards.len() < action_count {
        let mut card = candidates[rng.random_range(0..candidates.len())].clone();
        card.target_uid = target_uids
            .get(rng.random_range(0..target_uids.len()))
            .copied();
        cards.push(card);
    }
    cards
}

fn select_skill(
    entity: &sonettobuf::FightEntityInfo,
    ex_point: &ExPointManager,
    eureka: &EurekaManager,
) -> Option<i32> {
    let ultimate = entity.ex_skill.filter(|skill_id| *skill_id > 0);
    let uid = entity.uid.unwrap_or_default();
    let boss_power = eureka.get(uid, PowerType::ZongMaoBossEnergy.id());
    if boss_power.max > 0 {
        return if boss_power.is_full() {
            ultimate
        } else {
            entity
                .skill_group1
                .first()
                .or(entity.skill_group2.first())
                .copied()
        };
    }

    let current_ex_point = ex_point.get(uid);
    let required = ultimate
        .map(crate::engine::skill::effect::catalog::configured_big_skill_point)
        .filter(|cost| *cost > 0)
        .unwrap_or(5)
        .saturating_add(entity.expoint_max_add.unwrap_or_default());
    if let Some(ultimate) = ultimate
        && current_ex_point >= required
    {
        return Some(ultimate);
    }

    entity
        .skill_group1
        .first()
        .or(entity.skill_group2.first())
        .copied()
}
