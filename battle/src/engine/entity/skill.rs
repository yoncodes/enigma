use sonettobuf::CardInfo;
use std::collections::HashMap;

use super::input::HeroBuildInput;

pub struct Skill;

impl Skill {
    pub fn get(
        hero: &HeroBuildInput,
        is_sub: bool,
        destiny: Option<&HashMap<i32, i32>>,
    ) -> (Vec<i32>, Vec<i32>) {
        let (mut sg1, mut sg2) = if is_sub {
            (
                Self::get_from_character(hero.hero_id, 1),
                Self::get_from_character(hero.hero_id, 2),
            )
        } else {
            Self::get_skill_groups_with_destiny(hero.hero_id, hero.ex_skill_level, None)
        };

        if let Some(map) = destiny {
            Self::apply_exchange(&mut sg1, map);
            Self::apply_exchange(&mut sg2, map);
        }

        (sg1, sg2)
    }

    pub fn get_ex(hero: &HeroBuildInput, destiny: Option<&HashMap<i32, i32>>) -> i32 {
        let ex = Self::active_skills(hero.hero_id, hero.ex_skill_level).2;

        destiny.and_then(|map| map.get(&ex).copied()).unwrap_or(ex)
    }

    pub fn get_skill_groups_with_destiny(
        hero_id: i32,
        ex_level: i32,
        destiny: Option<&HashMap<i32, i32>>,
    ) -> (Vec<i32>, Vec<i32>) {
        let (mut sg1, mut sg2, _) = Self::for_loadout(hero_id, ex_level);

        if let Some(map) = destiny {
            Self::apply_exchange(&mut sg1, map);
            Self::apply_exchange(&mut sg2, map);
        }

        (sg1, sg2)
    }

    pub fn for_loadout(hero_id: i32, ex_level: i32) -> (Vec<i32>, Vec<i32>, i32) {
        Self::active_skills(hero_id, ex_level)
    }

    fn get_from_character(hero_id: i32, group: i32) -> Vec<i32> {
        let game = config::configs::get();
        let Some(character) = game.character.get(hero_id) else {
            tracing::warn!("Character {} not found", hero_id);
            return vec![];
        };
        parse_skill_group(&character.skill, group)
    }

    fn active_skills(hero_id: i32, ex_level: i32) -> (Vec<i32>, Vec<i32>, i32) {
        let game = config::configs::get();
        let Some(character) = game.character.get(hero_id) else {
            tracing::warn!(hero_id, "character not found while resolving active skills");
            return Default::default();
        };
        let mut group1 = parse_skill_group(&character.skill, 1);
        let mut group2 = parse_skill_group(&character.skill, 2);
        let mut ex_skill = character.ex_skill;
        let mut upgrades = game
            .skill_ex_level
            .iter()
            .filter(|row| row.hero_id == hero_id && row.skill_level <= ex_level)
            .collect::<Vec<_>>();
        upgrades.sort_by_key(|row| row.skill_level);
        for upgrade in upgrades {
            if !upgrade.skill_group1.trim().is_empty() {
                group1 = configured_skill_ids(&upgrade.skill_group1);
            }
            if !upgrade.skill_group2.trim().is_empty() {
                group2 = configured_skill_ids(&upgrade.skill_group2);
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

fn configured_skill_ids(raw: &str) -> Vec<i32> {
    raw.split(|character: char| !character.is_ascii_digit() && character != '-')
        .filter_map(|part| part.parse().ok())
        .filter(|skill_id| config::configs::get().skill.get(*skill_id).is_some())
        .collect()
}

pub fn parse_skill_group(skill_str: &str, target_group: i32) -> Vec<i32> {
    for group_str in skill_str.split('|') {
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
    config::try_get()
        .and_then(|db| db.skill.get(skill_id))
        .map(|row| row.skill_rank)
        .unwrap_or_default()
}

pub fn card_skill_rank(card: &CardInfo) -> i32 {
    card.skill_id
        .and_then(|skill_id| config::try_get().and_then(|db| db.skill.get(skill_id)))
        .map(|row| row.skill_rank)
        .unwrap_or_else(|| card.card_effect.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::{Skill, parse_skill_group};
    use crate::test_support::init_config;

    #[test]
    fn parses_requested_skill_group() {
        assert_eq!(parse_skill_group("1#10#11|2#20#21", 2), vec![20, 21]);
    }

    #[test]
    fn selected_level_replaces_active_skills_and_ultimate_cumulatively() {
        init_config();

        assert_eq!(
            Skill::active_skills(3134, 5),
            (
                vec![31345111, 31345112, 31345113],
                vec![31344121, 31344122, 31344123],
                31345131,
            )
        );
    }
}
