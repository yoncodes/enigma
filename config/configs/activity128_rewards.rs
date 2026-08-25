// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity128Rewards {
    #[serde(rename = "activityId")]
    pub activity_id: i32,
    pub display: i32,
    pub id: i32,
    pub reward: String,
    #[serde(rename = "rewardPointNum")]
    pub reward_point_num: i32,
    pub stage: i32,
}
pub struct Activity128RewardsTable {
    records: Vec<Activity128Rewards>,
}

impl Activity128RewardsTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity128Rewards> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[Activity128Rewards] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity128Rewards> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}