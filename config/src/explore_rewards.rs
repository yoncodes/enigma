//! Exploration rewards extracted from the matching client configuration.

use serde::Deserialize;
use std::{collections::HashSet, sync::OnceLock};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chest {
    pub id: i32,
    pub episode_id: i32,
    pub chapter_id: i32,
    pub bonus: String,
    pub is_count: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: i32,
    #[serde(rename = "type")]
    pub item_type: i32,
    pub is_stackable: bool,
    pub is_client_stackable: bool,
    pub effect: String,
    pub interact_effect: String,
    pub is_reserve: bool,
}

#[derive(Debug, Deserialize)]
pub struct Rewards {
    pub chests: Vec<Chest>,
    pub items: Vec<Item>,
}

static REWARDS: OnceLock<Result<Rewards, String>> = OnceLock::new();

pub fn rewards() -> anyhow::Result<&'static Rewards> {
    match REWARDS.get_or_init(|| {
        let rewards: Rewards = serde_json::from_str(include_str!("../data/explore_rewards.json"))
            .map_err(|error| format!("invalid exploration rewards: {error}"))?;
        let mut ids = HashSet::new();
        if rewards.chests.iter().any(|chest| !ids.insert(chest.id)) {
            return Err("duplicate exploration chest id".into());
        }
        ids.clear();
        if rewards.items.iter().any(|item| !ids.insert(item.id)) {
            return Err("duplicate exploration item id".into());
        }
        Ok(rewards)
    }) {
        Ok(rewards) => Ok(rewards),
        Err(error) => Err(anyhow::anyhow!("{error}")),
    }
}

pub fn chest(id: i32) -> anyhow::Result<Option<&'static Chest>> {
    Ok(rewards()?.chests.iter().find(|row| row.id == id))
}

pub fn item(id: i32) -> anyhow::Result<Option<&'static Item>> {
    Ok(rewards()?.items.iter().find(|row| row.id == id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_chests_and_exploration_items_load() {
        let rewards = rewards().unwrap();
        assert_eq!(rewards.chests.len(), 43);
        assert_eq!(rewards.items.len(), 4);
        let first = chest(1).unwrap().unwrap();
        assert_eq!((first.episode_id, first.chapter_id, first.is_count), (140101, 1401, 1));
        assert!(first.bonus.contains("2#2#20"));
        let item_chest = chest(1001).unwrap().unwrap();
        assert_eq!((item_chest.is_count, item_chest.bonus.as_str()), (0, "15#150002#1"));
        assert_eq!(item(150002).unwrap().unwrap().effect, "2");
        assert!(item(-1).unwrap().is_none());
    }
}
