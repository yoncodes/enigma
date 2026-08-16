// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcadeDifficulty {
    #[serde(rename = "addSkill")]
    pub add_skill: i32,
    #[serde(rename = "characterTurnTime")]
    pub character_turn_time: i32,
    pub level: i32,
    pub scope: String,
}
pub struct ArcadeDifficultyTable {
    records: Vec<ArcadeDifficulty>,
}

impl ArcadeDifficultyTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<ArcadeDifficulty> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[ArcadeDifficulty] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, ArcadeDifficulty> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}