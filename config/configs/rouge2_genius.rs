// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rouge2Genius {
    pub desc: String,
    pub effects: String,
    pub icon: String,
    pub id: i32,
    pub name: String,
    #[serde(rename = "overviewText")]
    pub overview_text: String,
    #[serde(rename = "replaceId")]
    pub replace_id: String,
    #[serde(rename = "talentOrder")]
    pub talent_order: i32,
    #[serde(rename = "talentType")]
    pub talent_type: i32,
    #[serde(rename = "typeName")]
    pub type_name: String,
}
use std::collections::HashMap;

pub struct Rouge2GeniusTable {
    records: Vec<Rouge2Genius>,
    by_id: HashMap<i32, usize>,
}

impl Rouge2GeniusTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Rouge2Genius> = crate::load_rows(path)?;

        let mut by_id = HashMap::with_capacity(records.len());

        for (idx, record) in records.iter().enumerate() {
            by_id.insert(record.id, idx);
        }

        Ok(Self { records, by_id })
    }

    #[inline]
    pub fn get(&self, id: i32) -> Option<&Rouge2Genius> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[Rouge2Genius] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Rouge2Genius> {
        self.records.iter()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}
