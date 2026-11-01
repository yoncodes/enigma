// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TowerComposePointLevel {
    #[serde(rename = "bossPointBase")]
    pub boss_point_base: i32,
    pub level: i32,
}
pub struct TowerComposePointLevelTable {
    records: Vec<TowerComposePointLevel>,
}

impl TowerComposePointLevelTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<TowerComposePointLevel> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[TowerComposePointLevel] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, TowerComposePointLevel> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}