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
        target::{TargetContext, TargetPool, TargetResolver},
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

pub(crate) const AI_HAND_CARDS_PER_ENEMY: usize = 2;

pub(crate) fn ai_hand_deal<R: Rng + ?Sized>(
    fight: &Fight,
    hand: &[CardInfo],
    rng: &mut R,
) -> Vec<CardInfo> {
    let mut dealt = Vec::new();
    for entity in active_enemy_entities(fight) {
        let groups = [entity.skill_group1.first(), entity.skill_group2.first()]
            .into_iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        if groups.is_empty() {
            continue;
        }
        let held = hand.iter().filter(|card| card.uid == entity.uid).count();
        for _ in held..AI_HAND_CARDS_PER_ENEMY {
            dealt.extend(card_for(
                entity,
                Some(groups[rng.random_range(0..groups.len())]),
            ));
        }
    }
    dealt
}

pub(crate) struct AiHandQueue {
    pub queue: Vec<CardInfo>,
    pub from_hand: Vec<CardInfo>,
}

pub(crate) fn ai_hand_queue<R: Rng + ?Sized>(
    fight: &Fight,
    hand: &[CardInfo],
    ex_point: &ExPointManager,
    eureka: &EurekaManager,
    extra_actions: i32,
    rng: &mut R,
) -> AiHandQueue {
    let target_uids = active_player_uids(fight);
    let mut queue = Vec::new();
    let mut from_hand = Vec::new();
    let mut taken = vec![false; hand.len()];
    if target_uids.is_empty() {
        return AiHandQueue { queue, from_hand };
    }
    for entity in active_enemy_entities(fight) {
        let selected = select_skill(entity, ex_point, eureka);
        let ultimate = entity.ex_skill.filter(|skill_id| *skill_id > 0);
        let held = (selected != ultimate)
            .then(|| (0..hand.len()).find(|&index| !taken[index] && hand[index].uid == entity.uid))
            .flatten();
        let card = match held {
            Some(index) => {
                taken[index] = true;
                from_hand.push(hand[index].clone());
                Some(hand[index].clone())
            }
            None => card_for(entity, selected),
        };
        queue.extend(card);
    }
    let candidates = queue.clone();
    let action_count = action_count(queue.len(), extra_actions);
    queue.truncate(action_count);
    from_hand.retain(|card| queue.iter().any(|queued| queued == card));
    while !candidates.is_empty() && queue.len() < action_count {
        queue.push(candidates[rng.random_range(0..candidates.len())].clone());
    }
    for card in &mut queue {
        card.target_uid = target_uids
            .get(rng.random_range(0..target_uids.len()))
            .copied();
    }
    AiHandQueue { queue, from_hand }
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
    let pool = TargetPool::from_fight_with_catalog(managers.catalog(), fight);
    resolve_targets(
        &pool,
        managers,
        catalog,
        determinism,
        TargetContext {
            battle_id: fight.battle_id.unwrap_or_default(),
            current_round: fight.cur_round.unwrap_or_default(),
            ..Default::default()
        },
        cards,
    )
}

pub(crate) fn resolve_targets(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
    context: TargetContext,
    cards: &mut [CardInfo],
) -> Result<(), String> {
    for card in cards {
        let source_uid = card
            .uid
            .filter(|uid| *uid != 0)
            .ok_or_else(|| "generated AI card has no owner".to_owned())?;
        let runtime_pool = pool.runtime_view_including(managers, Some(source_uid));
        let skill_id = card
            .skill_id
            .filter(|skill_id| *skill_id > 0)
            .ok_or_else(|| "generated AI card has no skill".to_owned())?;
        if catalog.get(skill_id).is_none() {
            return Err(format!(
                "generated AI skill {skill_id} is missing from the effect catalog"
            ));
        }
        let request = catalog.logic_request(skill_id);
        if request.code == 0 {
            return Err(format!(
                "generated AI skill {skill_id} has no configured target"
            ));
        }
        let code = request.code;
        let attack = catalog.is_attack(skill_id);
        let candidates = TargetResolver::resolve_primary_candidates(
            request,
            skill_id,
            source_uid,
            &runtime_pool,
            determinism,
            Some(managers),
            TargetContext {
                runtime_target_uid: card.target_uid.unwrap_or_default(),
                active_skill_id: skill_id,
                active_skill_source_uid: source_uid,
                active_skill_is_attack: attack,
                active_skill_rank: managers.catalog().skill_rank(skill_id),
                active_skill_type: managers.catalog().damage_target_count_kind(code),
                active_skill_effect_tag: catalog.effect_tag(skill_id),
                damage_target_count_kind: managers.catalog().damage_target_count_kind(code),
                ..context
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
