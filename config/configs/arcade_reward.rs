// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcadeReward {
    #[serde(rename = "activityId")]
    pub activity_id: i32,
    pub id: i32,
    pub reward: i32,
    pub score: i32,
    pub special: i32,
}
use std::collections::HashMap;

pub struct ArcadeRewardTable {
    records: Vec<ArcadeReward>,
    by_id: HashMap<i32, usize>,
}

impl ArcadeRewardTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<ArcadeReward> = crate::load_rows(path)?;

        let mut by_id = HashMap::with_capacity(records.len());

        for (idx, record) in records.iter().enumerate() {
            by_id.insert(record.id, idx);
        }

        Ok(Self {
            records,
            by_id,
        })
    }

    #[inline]
    pub fn get(&self, id: i32) -> Option<&ArcadeReward> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[ArcadeReward] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, ArcadeReward> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}