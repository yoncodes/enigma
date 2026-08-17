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
        Self::groups(
            crate::catalog::BattleCatalog::global().game_data(),
            hero,
            is_sub,
            destiny,
        )
    }

    pub(crate) fn loadout(
        game: &config::GameDB,
        hero: &HeroBuildInput,
        is_sub: bool,
        destiny: Option<&HashMap<i32, i32>>,
    ) -> (Vec<i32>, Vec<i32>, i32) {
        let (group1, group2) = Self::groups(game, hero, is_sub, destiny);
        let ex_skill = Self::ex(game, hero, destiny);
        (group1, group2, ex_skill)
    }

    fn groups(
        game: &config::GameDB,
        hero: &HeroBuildInput,
        is_sub: bool,
        destiny: Option<&HashMap<i32, i32>>,
    ) -> (Vec<i32>, Vec<i32>) {
        let (mut sg1, mut sg2) = if is_sub {
            (
                Self::get_from_character(game, hero.hero_id, 1),
                Self::get_from_character(game, hero.hero_id, 2),
            )
        } else {
            let (group1, group2, _) = Self::active_skills(game, hero.hero_id, hero.ex_skill_level);
            (group1, group2)
        };
        if let Some(map) = destiny {
            Self::apply_exchange(&mut sg1, map);
            Self::apply_exchange(&mut sg2, map);
        }

        (sg1, sg2)
    }

    pub fn get_ex(hero: &HeroBuildInput, destiny: Option<&HashMap<i32, i32>>) -> i32 {
        Self::ex(
            crate::catalog::BattleCatalog::global().game_data(),
            hero,
            destiny,
        )
    }

    fn ex(
        game: &config::GameDB,
        hero: &HeroBuildInput,
        destiny: Option<&HashMap<i32, i32>>,
    ) -> i32 {
        let ex = Self::active_skills(game, hero.hero_id, hero.ex_skill_level).2;
        destiny.and_then(|map| map.get(&ex).copied()).unwrap_or(ex)
    }

    pub fn get_skill_groups_with_destiny(
        hero_id: i32,
        ex_level: i32,
        destiny: Option<&HashMap<i32, i32>>,
    ) -> (Vec<i32>, Vec<i32>) {
        let game = crate::catalog::BattleCatalog::global().game_data();
        let (mut sg1, mut sg2, _) = Self::active_skills(game, hero_id, ex_level);

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
            ex_level,
        )
    }

    fn get_from_character(game: &config::GameDB, hero_id: i32, group: i32) -> Vec<i32> {
        let Some(character) = game.character.get(hero_id) else {
            tracing::warn!("Character {} not found", hero_id);
            return vec![];
        };
        parse_skill_group(&character.skill, group)
    }

    pub(crate) fn active_skills(
        game: &config::GameDB,
        hero_id: i32,
        ex_level: i32,
    ) -> (Vec<i32>, Vec<i32>, i32) {
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
mod tests {
    use super::{Skill, parse_skill_group};
    use crate::test_support::init_config;

    #[test]
    fn parses_requested_skill_group() {
        assert_eq!(parse_skill_group("1#10#11|2#20#21", 2), vec![20, 21]);
    }

    #[test]
    fn parses_only_the_first_choice_family() {
        assert_eq!(
            parse_skill_group("1#10#11#12,20#21#22|2#30#31", 1),
            vec![10, 11, 12]
        );
    }

    #[test]
    fn nautika_portrayal_tiers_keep_only_the_primary_choice_family() {
        init_config();
        let game = crate::test_support::game_data();
        for (level, expected) in [
            (1, vec![312001213, 312001223, 312001233]),
            (3, vec![312001214, 312001224, 312001234]),
            (4, vec![312001215, 312001225, 312001235]),
        ] {
            let (_, group2, _) = Skill::active_skills(game, 3120, level);
            assert_eq!(group2, expected, "portrayal level {level}");
        }
    }

    #[test]
    fn paper_heron_initial_loadout_keeps_only_the_primary_choice_family() {
        init_config();
        let game = crate::test_support::game_data();
        assert_eq!(
            Skill::active_skills(game, 3135, 0),
            (
                vec![31350111, 31350112, 31350113],
                vec![31350121, 31350122, 31350123],
                31350131,
            )
        );
        assert_eq!(
            Skill::active_skills(game, 3135, 1).0,
            vec![31350114, 31350115, 31350116]
        );
    }

    #[test]
    fn selected_level_replaces_active_skills_and_ultimate_cumulatively() {
        init_config();

        assert_eq!(
            Skill::active_skills(crate::test_support::game_data(), 3134, 5),
            (
                vec![31345111, 31345112, 31345113],
                vec![31344121, 31344122, 31344123],
                31345131,
            )
        );
    }
}
