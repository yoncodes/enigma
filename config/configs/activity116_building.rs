// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity116Building {
    #[serde(rename = "buildingType")]
    pub building_type: i32,
    #[serde(rename = "configType")]
    pub config_type: String,
    pub cost: String,
    pub desc: String,
    #[serde(rename = "elementId")]
    pub element_id: i32,
    #[serde(rename = "filterEpisode")]
    pub filter_episode: String,
    pub icon: String,
    pub id: i32,
    pub level: i32,
    #[serde(rename = "lightBgUrl")]
    pub light_bg_url: String,
    pub name: String,
}
use std::collections::HashMap;

pub struct Activity116BuildingTable {
    records: Vec<Activity116Building>,
    by_id: HashMap<i32, usize>,
}

impl Activity116BuildingTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity116Building> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&Activity116Building> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[Activity116Building] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity116Building> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}