use rand::Rng;
use sonettobuf::{CardInfo, Fight};

use crate::engine::{
    manager::{
        BattleManagers,
        eureka::{EurekaManager, PowerType},
        ex_point::ExPointManager,
    },
    runtime::determinism::RoundDeterminism,
    skill::{
        effect::SkillEffectCatalog,
        target::{TargetContext, TargetPool, TargetRequest, TargetResolver},
    },
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
    let mut cards = selectable_cards(enemies, ex_point, eureka)
        .into_iter()
        .map(|mut card| {
            card.target_uid = target_uids
                .get(rng.random_range(0..target_uids.len()))
                .copied();
            card
        })
        .collect::<Vec<_>>();
    let candidates = cards.clone();
    let action_count = action_count(cards.len(), extra_actions);
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

pub(crate) fn generated_ai_action_count(
    fight: &Fight,
    ex_point: &ExPointManager,
    eureka: &EurekaManager,
    extra_actions: i32,
) -> usize {
    if active_player_uids(fight).is_empty() {
        return 0;
    }
    action_count(
        selectable_cards(active_enemy_entities(fight), ex_point, eureka).len(),
        extra_actions,
    )
}

pub(crate) fn resolve_configured_targets(
    fight: &Fight,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
    cards: &mut [CardInfo],
) -> Result<(), String> {
    let pool =
        TargetPool::from_fight_with_catalog(managers.catalog(), fight).runtime_view(managers);
    for card in cards {
        let source_uid = card
            .uid
            .filter(|uid| *uid != 0)
            .ok_or_else(|| "generated AI card has no owner".to_owned())?;
        let skill_id = card
            .skill_id
            .filter(|skill_id| *skill_id > 0)
            .ok_or_else(|| "generated AI card has no skill".to_owned())?;
        if catalog.get(skill_id).is_none() {
            return Err(format!(
                "generated AI skill {skill_id} is missing from the effect catalog"
            ));
        }
        let code = catalog.logic_target(skill_id);
        if code == 0 {
            return Err(format!(
                "generated AI skill {skill_id} has no configured target"
            ));
        }
        let attack = catalog.is_attack(skill_id);
        let candidates = TargetResolver::resolve_primary_candidates(
            &TargetRequest {
                code,
                raw: Vec::new(),
            },
            skill_id,
            source_uid,
            &pool,
            determinism,
            Some(managers),
            TargetContext {
                runtime_target_uid: card.target_uid.unwrap_or_default(),
                active_skill_id: skill_id,
                active_skill_source_uid: source_uid,
                active_skill_is_attack: attack,
                active_skill_rank: managers.catalog().skill_rank(skill_id),
                active_skill_type: catalog.skill_type(skill_id),
                active_skill_effect_tag: catalog.effect_tag(skill_id),
                damage_target_count_kind: managers.catalog().damage_target_count_kind(code),
                battle_id: fight.battle_id.unwrap_or_default(),
                current_round: fight.cur_round.unwrap_or_default(),
                ..Default::default()
            },
        );
        let target_uid = card
            .target_uid
            .filter(|target_uid| candidates.contains(target_uid))
            .or_else(|| candidates.first().copied())
            .ok_or_else(|| {
                format!(
                    "generated AI skill {skill_id} has no configured target for owner {source_uid}"
                )
            })?;
        card.target_uid = Some(target_uid);
    }
    Ok(())
}

fn selectable_cards(
    enemies: Vec<&sonettobuf::FightEntityInfo>,
    ex_point: &ExPointManager,
    eureka: &EurekaManager,
) -> Vec<CardInfo> {
    enemies
        .into_iter()
        .filter_map(|entity| card_for(entity, select_skill(entity, ex_point, eureka)))
        .collect()
}

fn action_count(candidate_count: usize, extra_actions: i32) -> usize {
    if candidate_count == 0 {
        return 0;
    }
    i32::try_from(candidate_count)
        .unwrap_or(i32::MAX)
        .saturating_add(extra_actions)
        .max(0) as usize
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
