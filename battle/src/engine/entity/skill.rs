use sonettobuf::CardInfo;
use std::collections::HashMap;

use super::{destiny::Destiny, input::HeroBuildInput};

pub struct Skill;

impl Skill {
    pub(crate) fn trial(
        game: &config::GameDB,
        trial: &config::hero_trial::HeroTrial,
        rank: i32,
    ) -> (Vec<i32>, Vec<i32>, i32) {
        let (mut group1, mut group2, mut ultimate) =
            Self::active_skills(game, trial.hero_id, rank, trial.ex_skill_lv);
        if let Some(exchanges) = Destiny::trial_exchanges(game, trial.facets_id, trial.facetslevel)
        {
            Self::apply_exchange(&mut group1, &exchanges);
            Self::apply_exchange(&mut group2, &exchanges);
            Self::apply_exchange(std::slice::from_mut(&mut ultimate), &exchanges);
        }
        (group1, group2, ultimate)
    }

    pub(crate) fn loadout(
        game: &config::GameDB,
        hero: &HeroBuildInput,
        destiny: Option<&HashMap<i32, i32>>,
    ) -> (Vec<i32>, Vec<i32>, i32) {
        let (group1, group2) = Self::groups(game, hero, destiny);
        let ex_skill = Self::ex(game, hero, destiny);
        (group1, group2, ex_skill)
    }

    fn groups(
        game: &config::GameDB,
        hero: &HeroBuildInput,
        destiny: Option<&HashMap<i32, i32>>,
    ) -> (Vec<i32>, Vec<i32>) {
        let (mut sg1, mut sg2) = if let Some(gear) =
            game.equipped_gear(hero.hero_id, &hero.extra_str, hero.ex_skill_level)
        {
            (
                configured_skill_ids(game, &gear.skill_group1),
                configured_skill_ids(game, &gear.skill_group2),
            )
        } else {
            let (group1, group2, _) =
                Self::active_skills(game, hero.hero_id, hero.rank, hero.ex_skill_level);
            (group1, group2)
        };
        if let Some(map) = destiny {
            Self::apply_exchange(&mut sg1, map);
            Self::apply_exchange(&mut sg2, map);
        }
        let talents = game.talent_exchanges(hero.hero_id, &hero.extra_str, hero.ex_skill_level);
        for id in sg1.iter_mut().chain(sg2.iter_mut()) {
            *id = talent_exchange(*id, &talents);
        }

        (sg1, sg2)
    }

    fn ex(
        game: &config::GameDB,
        hero: &HeroBuildInput,
        destiny: Option<&HashMap<i32, i32>>,
    ) -> i32 {
        let ex = game
            .equipped_gear(hero.hero_id, &hero.extra_str, hero.ex_skill_level)
            .map(|gear| gear.skill_ex)
            .unwrap_or_else(|| {
                Self::active_skills(game, hero.hero_id, hero.rank, hero.ex_skill_level).2
            });
        let ex = destiny.and_then(|map| map.get(&ex).copied()).unwrap_or(ex);
        talent_exchange(
            ex,
            &game.talent_exchanges(hero.hero_id, &hero.extra_str, hero.ex_skill_level),
        )
    }

    pub fn get_skill_groups_with_destiny(
        hero_id: i32,
        ex_level: i32,
        destiny: Option<&HashMap<i32, i32>>,
    ) -> (Vec<i32>, Vec<i32>) {
        let game = crate::catalog::BattleCatalog::global().game_data();
        let (mut sg1, mut sg2, _) = Self::active_skills(game, hero_id, 0, ex_level);

        if let Some(map) = destiny {
            Self::apply_exchange(&mut sg1, map);
            Self::apply_exchange(&mut sg2, map);
        }

        (sg1, sg2)
    }

    pub fn for_loadout(hero_id: i32, ex_level: i32) -> (Vec<i32>, Vec<i32>, i32) {
        Self::active_skills(
            crate::catalog::BattleCatalog::global().game_data(),
            hero_id,
            0,
            ex_level,
        )
    }

    pub(crate) fn active_skills(
        game: &config::GameDB,
        hero_id: i32,
        rank: i32,
        ex_level: i32,
    ) -> (Vec<i32>, Vec<i32>, i32) {
        let Some(character) = game.character.get(hero_id) else {
            tracing::warn!(hero_id, "character not found while resolving active skills");
            return Default::default();
        };
        let rank_replace = game.character_rank_replace.get(hero_id);
        let (skill, mut ex_skill) = match rank_replace {
            Some(replace) if rank > 2 => (replace.skill.as_str(), replace.ex_skill),
            _ => (character.skill.as_str(), character.ex_skill),
        };
        let mut group1 = parse_skill_group(skill, 1);
        let mut group2 = parse_skill_group(skill, 2);
        // Portraits upgrade a rank-replaced kit only after the replacement.
        if rank_replace.is_some() && rank <= 2 {
            return (group1, group2, ex_skill);
        }
        let mut upgrades = game
            .skill_ex_level
            .iter()
            .filter(|row| row.hero_id == hero_id && row.skill_level <= ex_level)
            .collect::<Vec<_>>();
        upgrades.sort_by_key(|row| row.skill_level);
        for upgrade in upgrades {
            if !upgrade.skill_group1.trim().is_empty() {
                group1 = configured_skill_ids(game, &upgrade.skill_group1);
            }
            if !upgrade.skill_group2.trim().is_empty() {
                group2 = configured_skill_ids(game, &upgrade.skill_group2);
            }
            if upgrade.skill_ex != 0 {
                ex_skill = upgrade.skill_ex;
            }
        }
        (group1, group2, ex_skill)
    }

    fn apply_exchange(list: &mut [i32], map: &HashMap<i32, i32>) {
        for value in list.iter_mut() {
            if let Some(new) = map.get(value) {
                *value = *new;
            }
        }
    }
}

/// Talent exchanges apply in order, so a later one can remap an id an earlier
/// one produced.
pub(crate) fn talent_exchange(id: i32, exchanges: &[(i32, i32)]) -> i32 {
    exchanges
        .iter()
        .fold(id, |id, (from, to)| if id == *from { *to } else { id })
}

fn configured_skill_ids(game: &config::GameDB, raw: &str) -> Vec<i32> {
    raw.split(',')
        .next()
        .unwrap_or_default()
        .split(|character: char| !character.is_ascii_digit() && character != '-')
        .filter_map(|part| part.parse().ok())
        .filter(|skill_id| game.skill.get(*skill_id).is_some())
        .collect()
}

pub fn parse_skill_group(skill_str: &str, target_group: i32) -> Vec<i32> {
    for group_str in skill_str.split('|') {
        let group_str = group_str.split(',').next().unwrap_or(group_str);
        let mut parts = group_str.split('#');
        let Some(first) = parts.next() else { continue };
        let Ok(group_num) = first.parse::<i32>() else {
            continue;
        };

        if group_num == target_group {
            return parts.filter_map(|s| s.parse::<i32>().ok()).collect();
        }
    }
    vec![]
}

pub fn split_ids(value: &str) -> Vec<i32> {
    value
        .split(['#', '|', ','])
        .filter_map(|s| s.trim().parse::<i32>().ok())
        .collect()
}

pub fn skill_rank(skill_id: i32) -> i32 {
    crate::catalog::BattleCatalog::try_global()
        .map(|catalog| catalog.skill_rank(skill_id))
        .unwrap_or_default()
}

pub fn card_skill_rank(card: &CardInfo) -> i32 {
    crate::catalog::BattleCatalog::try_global()
        .map(|catalog| catalog.card_skill_rank(card))
        .unwrap_or_else(|| card.card_effect.unwrap_or_default())
}

#[cfg(test)]
mod tests;
