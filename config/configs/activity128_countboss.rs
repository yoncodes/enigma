// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity128Countboss {
    #[serde(rename = "battleId")]
    pub battle_id: i32,
    #[serde(rename = "finalMonsterId")]
    pub final_monster_id: String,
    #[serde(rename = "maxPoints")]
    pub max_points: i32,
    #[serde(rename = "monsterId")]
    pub monster_id: String,
}
pub struct Activity128CountbossTable {
    records: Vec<Activity128Countboss>,
}

impl Activity128CountbossTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity128Countboss> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[Activity128Countboss] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity128Countboss> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}