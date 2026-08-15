// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcadeCharacter {
    pub attack: i32,
    pub bomb: i32,
    pub category: String,
    pub collection: i32,
    pub defense: i32,
    pub desc: String,
    #[serde(rename = "hpCap")]
    pub hp_cap: i32,
    #[serde(rename = "hpPos")]
    pub hp_pos: Vec<f32>,
    pub icon: String,
    pub icon2: String,
    #[serde(rename = "icon2Offset")]
    pub icon2_offset: Vec<f32>,
    #[serde(rename = "icon2Offset2")]
    pub icon2_offset2: Vec<i32>,
    #[serde(rename = "icon2Scale")]
    pub icon2_scale: Vec<f32>,
    pub id: i32,
    #[serde(rename = "lockTip")]
    pub lock_tip: String,
    pub name: String,
    #[serde(rename = "resPath")]
    pub res_path: String,
    pub scale: Vec<f32>,
    pub shape: String,
    pub skill: i32,
    #[serde(rename = "skillCost")]
    pub skill_cost: i32,
}
use std::collections::HashMap;

pub struct ArcadeCharacterTable {
    records: Vec<ArcadeCharacter>,
    by_id: HashMap<i32, usize>,
}

impl ArcadeCharacterTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<ArcadeCharacter> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&ArcadeCharacter> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[ArcadeCharacter] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, ArcadeCharacter> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}