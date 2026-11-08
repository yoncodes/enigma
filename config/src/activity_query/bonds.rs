use crate::GameDB;
use std::collections::{BTreeMap, BTreeSet};

const BATTLE_TAG_GROUP: i32 = 2;
const SPECIAL_GROUP: i32 = 3;
const SPECIAL_EXPAND_BOND_GROUP_ID: i32 = 10021;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity128BondHero {
    careers: Vec<i32>,
    battle_tags: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity128BondGroup {
    pub group_id: i32,
    pub max_active_num: usize,
    tag_type: i32,
    careers: Vec<i32>,
    battle_tags: Vec<i32>,
}

impl Activity128BondGroup {
    pub fn matches(&self, hero: &Activity128BondHero) -> bool {
        intersects(&self.careers, &hero.careers) || intersects(&self.battle_tags, &hero.battle_tags)
    }

    pub fn is_battle_tag_group(&self) -> bool {
        self.tag_type == BATTLE_TAG_GROUP
    }

    pub fn enables_added_bond(&self) -> bool {
        self.group_id == SPECIAL_EXPAND_BOND_GROUP_ID && self.tag_type == SPECIAL_GROUP
    }
}

impl GameDB {
    pub fn activity128_bond_groups(&self, activity_id: i32) -> Option<Vec<Activity128BondGroup>> {
        let mut rows = BTreeMap::<i32, Vec<_>>::new();
        for row in self
            .activity128_expand_bond
            .iter()
            .filter(|row| row.activity_id == activity_id)
        {
            rows.entry(row.group_id).or_default().push(row);
        }

        rows.into_iter()
            .map(|(group_id, rows)| {
                let tag_type = rows.first()?.tag_type;
                if rows.iter().any(|row| row.tag_type != tag_type) {
                    return None;
                }
                let max_active_num = rows.iter().map(|row| row.active_num).max()?;
                let max_active_num = usize::try_from(max_active_num)
                    .ok()
                    .filter(|max| *max > 0)?;
                let mut careers = BTreeSet::new();
                let mut battle_tags = BTreeSet::new();
                for row in rows {
                    careers.extend(parse_tags(&row.tag1, '|')?);
                    battle_tags.extend(parse_tags(&row.tag2, '|')?);
                }
                Some(Activity128BondGroup {
                    group_id,
                    max_active_num,
                    tag_type,
                    careers: careers.into_iter().collect(),
                    battle_tags: battle_tags.into_iter().collect(),
                })
            })
            .collect()
    }

    pub fn activity128_bond_hero(
        &self,
        hero_id: i32,
        destiny_stone: i32,
    ) -> Option<Activity128BondHero> {
        let hero = self.character.get(hero_id)?;
        let careers = match self.fight_effect_group.get(hero.career) {
            Some(row) => parse_tags(&row.career, '#')?,
            None => vec![hero.career],
        };
        let battle_tag = if destiny_stone == 0 {
            &hero.battle_tag
        } else {
            let stone = self.character_destiny_stone_cost(destiny_stone)?;
            if stone.tag.is_empty() {
                &hero.battle_tag
            } else {
                &stone.tag
            }
        };
        Some(Activity128BondHero {
            careers,
            battle_tags: parse_tags(battle_tag, '#')?,
        })
    }
}

fn parse_tags(value: &str, separator: char) -> Option<Vec<i32>> {
    value
        .split(separator)
        .filter(|tag| !tag.is_empty())
        .map(|tag| tag.parse().ok())
        .collect()
}

fn intersects(left: &[i32], right: &[i32]) -> bool {
    left.iter().any(|tag| right.contains(tag))
}

#[cfg(test)]
mod tests;
