// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcadeMonster {
    pub attack: i32,
    pub category: String,
    pub defense: i32,
    pub desc: String,
    pub drop: String,
    #[serde(rename = "hasCorpse")]
    pub has_corpse: bool,
    #[serde(rename = "hpCap")]
    pub hp_cap: i32,
    #[serde(rename = "hpPos")]
    pub hp_pos: Vec<f32>,
    pub icon: String,
    #[serde(rename = "iconOffset")]
    pub icon_offset: Vec<f32>,
    #[serde(rename = "iconOffset2")]
    pub icon_offset2: Vec<i32>,
    #[serde(rename = "iconScale")]
    pub icon_scale: Vec<f32>,
    #[serde(rename = "iconScale2")]
    pub icon_scale2: Vec<f32>,
    pub id: i32,
    #[serde(rename = "illustrationShow")]
    pub illustration_show: i32,
    #[serde(rename = "moveType")]
    pub move_type: String,
    pub name: String,
    #[serde(rename = "posOffset")]
    pub pos_offset: Option<serde_json::Value>,
    pub race: String,
    #[serde(rename = "resPath")]
    pub res_path: String,
    pub scale: Vec<f32>,
    pub shape: String,
    #[serde(rename = "skillIds")]
    pub skill_ids: String,
}
use std::collections::HashMap;

pub struct ArcadeMonsterTable {
    records: Vec<ArcadeMonster>,
    by_id: HashMap<i32, usize>,
}

impl ArcadeMonsterTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<ArcadeMonster> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&ArcadeMonster> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[ArcadeMonster] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, ArcadeMonster> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}