// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RacingReward {
    pub bonus: i32,
    #[serde(rename = "episodeId")]
    pub episode_id: i32,
    pub star: i32,
}
pub struct RacingRewardTable {
    records: Vec<RacingReward>,
}

impl RacingRewardTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<RacingReward> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[RacingReward] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, RacingReward> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}