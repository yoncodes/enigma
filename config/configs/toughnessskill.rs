// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Toughnessskill {
    #[serde(rename = "cdBuff")]
    pub cd_buff: i32,
    #[serde(rename = "iconBreak")]
    pub icon_break: String,
    #[serde(rename = "iconNormal")]
    pub icon_normal: String,
    #[serde(rename = "passiveSkill")]
    pub passive_skill: String,
    pub toughnessskill: i32,
}
pub struct ToughnessskillTable {
    records: Vec<Toughnessskill>,
}

impl ToughnessskillTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Toughnessskill> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[Toughnessskill] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Toughnessskill> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}