// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity233LvBonus {
    #[serde(rename = "bpId")]
    pub bp_id: i32,
    #[serde(rename = "freeBonus")]
    pub free_bonus: String,
    #[serde(rename = "keyBonus")]
    pub key_bonus: i32,
    pub level: i32,
    #[serde(rename = "payBonus")]
    pub pay_bonus: String,
}
pub struct Activity233LvBonusTable {
    records: Vec<Activity233LvBonus>,
}

impl Activity233LvBonusTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity233LvBonus> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[Activity233LvBonus] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity233LvBonus> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}