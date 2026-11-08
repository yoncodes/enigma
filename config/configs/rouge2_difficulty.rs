// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rouge2Difficulty {
    #[serde(rename = "balanceLevel")]
    pub balance_level: String,
    pub bg: String,
    pub desc: String,
    pub difficulty: i32,
    #[serde(rename = "initCollections")]
    pub init_collections: String,
    #[serde(rename = "monsterDesc")]
    pub monster_desc: String,
    #[serde(rename = "preDifficulty")]
    pub pre_difficulty: i32,
    #[serde(rename = "scoreReward")]
    pub score_reward: i32,
    #[serde(rename = "startView")]
    pub start_view: String,
    pub title: String,
    pub title_en: String,
}
pub struct Rouge2DifficultyTable {
    records: Vec<Rouge2Difficulty>,
}

impl Rouge2DifficultyTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Rouge2Difficulty> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[Rouge2Difficulty] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Rouge2Difficulty> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}