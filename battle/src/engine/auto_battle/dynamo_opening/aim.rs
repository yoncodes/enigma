use sonettobuf::CardInfo;

use crate::engine::{
    manager::{BattleManagers, card::CardManager},
    runtime::schedule::card_skill_is_blocked,
    skill::{
        effect::SkillEffectCatalog,
        target::{CardSelectionResolver, TargetPool},
    },
};

use super::{Candidate, channel, ordinary, team_buff};

pub(in crate::engine::auto_battle) fn aim(
    preparation: &Candidate,
    pool: &TargetPool,
    preferred_target: Option<i64>,
) -> Candidate {
    let mut preparation = preparation.clone();
    if let Some(target) =
        CardSelectionResolver::enemy_aim(preparation.source_uid, pool, preferred_target)
    {
        preparation.target_uid = target;
    }
    preparation
}

pub(in crate::engine::auto_battle) fn has_channel(
    cards: &CardManager,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    normal_ap: i32,
) -> bool {
    cards.visible_cards().any(|card| {
        let (Some(source_uid), Some(skill_id)) = (card.uid, card.skill_id) else {
            return false;
        };
        ordinary(card)
            && managers.catalog().card_skill_rank(card) == 1
            && cards.can_use_card(card)
            && channel(skill_id, catalog)
            && catalog.issues(skill_id).is_empty()
            && pool
                .entity(source_uid)
                .is_some_and(|source| source.current_hp > 0)
            && !card_skill_is_blocked(managers, catalog, source_uid, skill_id)
            && (1..=normal_ap).contains(&super::super::action_point_cost(
                card, source_uid, skill_id, managers, catalog,
            ))
    })
}

pub(in crate::engine::auto_battle) fn allows_aim(
    card: &CardInfo,
    skill_id: i32,
    target_uid: Option<i64>,
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
) -> bool {
    ordinary(card)
        && managers.catalog().card_skill_rank(card) == 1
        && card.skill_id == Some(skill_id)
        && (channel(skill_id, catalog) || team_buff(skill_id, catalog))
        && card.uid.zip(target_uid).is_some_and(|(source, target)| {
            CardSelectionResolver::is_enemy_aim(source, target, pool)
        })
}
