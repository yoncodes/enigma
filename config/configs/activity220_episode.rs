// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity220Episode {
    #[serde(rename = "activityId")]
    pub activity_id: i32,
    #[serde(rename = "disactiveEpisodeIds")]
    pub disactive_episode_ids: String,
    #[serde(rename = "episodeId")]
    pub episode_id: i32,
    #[serde(rename = "gameId")]
    pub game_id: i32,
    pub name: String,
    #[serde(rename = "preEpisodeBranchId")]
    pub pre_episode_branch_id: i32,
    #[serde(rename = "preEpisodeId")]
    pub pre_episode_id: i32,
    #[serde(rename = "storyBefore")]
    pub story_before: i32,
    #[serde(rename = "storyClear")]
    pub story_clear: i32,
    #[serde(rename = "type")]
    pub r#type: i32,
}
pub struct Activity220EpisodeTable {
    records: Vec<Activity220Episode>,
}

impl Activity220EpisodeTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity220Episode> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[Activity220Episode] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity220Episode> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}
