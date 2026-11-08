// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rouge2Formula {
    pub condition: String,
    pub details: String,
    pub effects: String,
    pub icon: String,
    pub id: i32,
    #[serde(rename = "isHide")]
    pub is_hide: i32,
    #[serde(rename = "mainIdNum")]
    pub main_id_num: String,
    pub name: String,
    pub rare: i32,
    #[serde(rename = "scoreReward")]
    pub score_reward: i32,
    pub sort: i32,
}
use std::collections::HashMap;

pub struct Rouge2FormulaTable {
    records: Vec<Rouge2Formula>,
    by_id: HashMap<i32, usize>,
}

impl Rouge2FormulaTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Rouge2Formula> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&Rouge2Formula> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[Rouge2Formula] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Rouge2Formula> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}