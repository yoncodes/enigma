// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Teaching {
    #[serde(rename = "battleTag")]
    pub battle_tag: String,
    pub bonus: String,
    pub detail: String,
    pub icon: String,
    pub id: i32,
    pub name: String,
    pub picture: String,
    #[serde(rename = "tagName")]
    pub tag_name: String,
}
use std::collections::HashMap;

pub struct TeachingTable {
    records: Vec<Teaching>,
    by_id: HashMap<i32, usize>,
}

impl TeachingTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Teaching> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&Teaching> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[Teaching] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Teaching> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}