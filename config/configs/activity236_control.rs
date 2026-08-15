// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity236Control {
    #[serde(rename = "conversionRate")]
    pub conversion_rate: String,
    #[serde(rename = "costId")]
    pub cost_id: i32,
    pub id: i32,
}
use std::collections::HashMap;

pub struct Activity236ControlTable {
    records: Vec<Activity236Control>,
    by_id: HashMap<i32, usize>,
}

impl Activity236ControlTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity236Control> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&Activity236Control> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[Activity236Control] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity236Control> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}
