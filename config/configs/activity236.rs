// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity236 {
    #[serde(rename = "activityId")]
    pub activity_id: i32,
    pub cost: i32,
    pub id: i32,
    pub reward: String,
    #[serde(rename = "showReward")]
    pub show_reward: String,
    #[serde(rename = "showVideo")]
    pub show_video: String,
    #[serde(rename = "showWindow")]
    pub show_window: String,
}
use std::collections::HashMap;

pub struct Activity236Table {
    records: Vec<Activity236>,
    by_id: HashMap<i32, usize>,
}

impl Activity236Table {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity236> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&Activity236> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[Activity236] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity236> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}
