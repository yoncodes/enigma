// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TowerComposeBossLv {
    #[serde(rename = "episodeId")]
    pub episode_id: i32,
    pub level: i32,
    #[serde(rename = "levelReq")]
    pub level_req: String,
    #[serde(rename = "templateChange")]
    pub template_change: String,
}
pub struct TowerComposeBossLvTable {
    records: Vec<TowerComposeBossLv>,
}

impl TowerComposeBossLvTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<TowerComposeBossLv> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[TowerComposeBossLv] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, TowerComposeBossLv> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}