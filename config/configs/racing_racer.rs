// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RacingRacer {
    #[serde(rename = "baseAcceleration")]
    pub base_acceleration: f32,
    #[serde(rename = "baseSpeed")]
    pub base_speed: i32,
    pub cost: i32,
    pub desc: String,
    pub desc1: String,
    pub difficult: i32,
    pub display: i32,
    #[serde(rename = "dolphinPrefab")]
    pub dolphin_prefab: String,
    pub id: i32,
    pub img: String,
    #[serde(rename = "initialEffect")]
    pub initial_effect: String,
    #[serde(rename = "maxSpeed")]
    pub max_speed: i32,
    pub name: String,
    pub pic: String,
    pub pos: String,
    #[serde(rename = "powerSpeed")]
    pub power_speed: i32,
    pub scale: String,
    #[serde(rename = "skillPic")]
    pub skill_pic: i32,
    pub team: String,
    #[serde(rename = "uiPic")]
    pub ui_pic: i32,
    #[serde(rename = "ultimateId")]
    pub ultimate_id: i32,
    #[serde(rename = "wrongPercent")]
    pub wrong_percent: f32,
}
use std::collections::HashMap;

pub struct RacingRacerTable {
    records: Vec<RacingRacer>,
    by_id: HashMap<i32, usize>,
}

impl RacingRacerTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<RacingRacer> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&RacingRacer> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[RacingRacer] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, RacingRacer> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}