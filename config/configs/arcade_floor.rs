// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcadeFloor {
    pub category: String,
    pub desc: String,
    pub icon: String,
    pub id: i32,
    #[serde(rename = "limitRound")]
    pub limit_round: i32,
    pub name: String,
    #[serde(rename = "posOffset")]
    pub pos_offset: Option<serde_json::Value>,
    pub priority: i32,
    #[serde(rename = "resPath")]
    pub res_path: String,
    pub scale: Vec<serde_json::Value>,
    pub shape: String,
    pub skill: i32,
}
use std::collections::HashMap;

pub struct ArcadeFloorTable {
    records: Vec<ArcadeFloor>,
    by_id: HashMap<i32, usize>,
}

impl ArcadeFloorTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<ArcadeFloor> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&ArcadeFloor> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[ArcadeFloor] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, ArcadeFloor> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}