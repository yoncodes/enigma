// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity128Episode {
    #[serde(rename = "activityId")]
    pub activity_id: i32,
    pub desc: String,
    #[serde(rename = "enhanceRole")]
    pub enhance_role: i32,
    #[serde(rename = "episodeId")]
    pub episode_id: i32,
    pub evaluate: String,
    pub layer: i32,
    #[serde(rename = "openDay")]
    pub open_day: i32,
    #[serde(rename = "recommendLevelDesc")]
    pub recommend_level_desc: String,
    pub stage: i32,
    #[serde(rename = "type")]
    pub r#type: i32,
}
pub struct Activity128EpisodeTable {
    records: Vec<Activity128Episode>,
}

impl Activity128EpisodeTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity128Episode> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[Activity128Episode] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity128Episode> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}