// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnbackReturnReward {
    pub bonus: String,
    pub id: i32,
    #[serde(rename = "isOnline")]
    pub is_online: i32,
    #[serde(rename = "lossDaysRange")]
    pub loss_days_range: i32,
    #[serde(rename = "needVitality")]
    pub need_vitality: i32,
    #[serde(rename = "turnbackId")]
    pub turnback_id: i32,
}
use std::collections::HashMap;

pub struct TurnbackReturnRewardTable {
    records: Vec<TurnbackReturnReward>,
    by_id: HashMap<i32, usize>,
}

impl TurnbackReturnRewardTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<TurnbackReturnReward> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&TurnbackReturnReward> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[TurnbackReturnReward] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, TurnbackReturnReward> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}