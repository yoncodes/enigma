// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity186Const {
    #[serde(rename = "activityId")]
    pub activity_id: i32,
    pub id: i32,
    pub value: String,
    pub value2: String,
}
use std::collections::HashMap;

pub struct Activity186ConstTable {
    records: Vec<Activity186Const>,
    by_id: HashMap<i32, usize>,
}

impl Activity186ConstTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity186Const> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&Activity186Const> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[Activity186Const] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity186Const> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}