// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity233Bp {
    #[serde(rename = "activityId")]
    pub activity_id: i32,
    #[serde(rename = "bpId")]
    pub bp_id: i32,
    #[serde(rename = "expLevelUp")]
    pub exp_level_up: i32,
    #[serde(rename = "unlockPremiumCost")]
    pub unlock_premium_cost: String,
}
pub struct Activity233BpTable {
    records: Vec<Activity233Bp>,
}

impl Activity233BpTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity233Bp> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[Activity233Bp] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity233Bp> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}