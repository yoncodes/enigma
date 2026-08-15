// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity128Level {
    pub bonus: String,
    #[serde(rename = "levelBg")]
    pub level_bg: String,
    #[serde(rename = "needExp")]
    pub need_exp: i32,
    #[serde(rename = "playerLevel")]
    pub player_level: i32,
    #[serde(rename = "spLevelBg")]
    pub sp_level_bg: i32,
}
pub struct Activity128LevelTable {
    records: Vec<Activity128Level>,
}

impl Activity128LevelTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity128Level> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[Activity128Level] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity128Level> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}
