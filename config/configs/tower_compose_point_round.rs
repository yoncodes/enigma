// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TowerComposePointRound {
    #[serde(rename = "bossPointAdd")]
    pub boss_point_add: i32,
    pub round: i32,
}
pub struct TowerComposePointRoundTable {
    records: Vec<TowerComposePointRound>,
}

impl TowerComposePointRoundTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<TowerComposePointRound> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[TowerComposePointRound] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, TowerComposePointRound> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}