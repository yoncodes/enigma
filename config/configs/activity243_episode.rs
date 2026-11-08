// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity243Episode {
    #[serde(rename = "activityId")]
    pub activity_id: i32,
    #[serde(rename = "clearBonus")]
    pub clear_bonus: i32,
    #[serde(rename = "episodeId")]
    pub episode_id: i32,
    #[serde(rename = "episodeType")]
    pub episode_type: i32,
    #[serde(rename = "gameId")]
    pub game_id: i32,
    pub name: String,
    #[serde(rename = "preEpisodeId")]
    pub pre_episode_id: i32,
}
pub struct Activity243EpisodeTable {
    records: Vec<Activity243Episode>,
}

impl Activity243EpisodeTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity243Episode> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[Activity243Episode] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity243Episode> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}