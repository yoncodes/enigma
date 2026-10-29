// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekwalkScene {
    #[serde(rename = "battleName")]
    pub battle_name: String,
    #[serde(rename = "buffId")]
    pub buff_id: i32,
    pub icon: String,
    pub id: i32,
    pub map: String,
    #[serde(rename = "mapId")]
    pub map_id: i32,
    pub name: String,
    pub name_en: String,
    #[serde(rename = "typeName")]
    pub type_name: String,
}
use std::collections::HashMap;

pub struct WeekwalkSceneTable {
    records: Vec<WeekwalkScene>,
    by_id: HashMap<i32, usize>,
}

impl WeekwalkSceneTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<WeekwalkScene> = crate::load_rows(path)?;

        let mut by_id = HashMap::with_capacity(records.len());

        for (idx, record) in records.iter().enumerate() {
            by_id.insert(record.id, idx);
        }

        Ok(Self { records, by_id })
    }

    #[inline]
    pub fn get(&self, id: i32) -> Option<&WeekwalkScene> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[WeekwalkScene] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, WeekwalkScene> {
        self.records.iter()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}
