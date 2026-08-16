// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DestinyFacetsExLevel {
    pub desc: String,
    #[serde(rename = "deviceId")]
    pub device_id: i32,
    #[serde(rename = "exchangeSkill")]
    pub exchange_skill: String,
    #[serde(rename = "heroId")]
    pub hero_id: i32,
    #[serde(rename = "passiveSkill")]
    pub passive_skill: String,
    #[serde(rename = "skillEx")]
    pub skill_ex: i32,
    #[serde(rename = "skillGroup1")]
    pub skill_group1: String,
    #[serde(rename = "skillGroup2")]
    pub skill_group2: String,
    #[serde(rename = "skillLevel")]
    pub skill_level: i32,
}
pub struct DestinyFacetsExLevelTable {
    records: Vec<DestinyFacetsExLevel>,
}

impl DestinyFacetsExLevelTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<DestinyFacetsExLevel> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[DestinyFacetsExLevel] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, DestinyFacetsExLevel> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}