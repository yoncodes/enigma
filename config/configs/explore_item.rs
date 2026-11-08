// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreItem {
    #[serde(rename = "audioId")]
    pub audio_id: i32,
    pub desc: String,
    pub desc2: String,
    pub effect: String,
    pub icon: String,
    pub id: i32,
    #[serde(rename = "interactEffect")]
    pub interact_effect: String,
    #[serde(rename = "isClientStackable")]
    pub is_client_stackable: bool,
    #[serde(rename = "isReserve")]
    pub is_reserve: bool,
    #[serde(rename = "isStackable")]
    pub is_stackable: bool,
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: i32,
}
use std::collections::HashMap;

pub struct ExploreItemTable {
    records: Vec<ExploreItem>,
    by_id: HashMap<i32, usize>,
}

impl ExploreItemTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<ExploreItem> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&ExploreItem> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[ExploreItem] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, ExploreItem> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}