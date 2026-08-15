// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity239 {
    #[serde(rename = "activityId")]
    pub activity_id: i32,
    pub bonus: String,
    pub id: i32,
    #[serde(rename = "openTime")]
    pub open_time: String,
    pub pic: String,
    #[serde(rename = "preId")]
    pub pre_id: i32,
    pub text: String,
    pub title: String,
}
use std::collections::HashMap;

pub struct Activity239Table {
    records: Vec<Activity239>,
    by_id: HashMap<i32, usize>,
}

impl Activity239Table {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity239> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&Activity239> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[Activity239] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity239> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}
