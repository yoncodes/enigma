// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RacingGift {
    #[serde(rename = "activityId")]
    pub activity_id: i32,
    pub cost: i32,
    pub desc: String,
    pub effect: String,
    pub gift_point: i32,
    pub group: i32,
    pub icon: String,
    pub level: i32,
    pub name: String,
    pub order: i32,
    #[serde(rename = "shortDesc")]
    pub short_desc: String,
}
pub struct RacingGiftTable {
    records: Vec<RacingGift>,
}

impl RacingGiftTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<RacingGift> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[RacingGift] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, RacingGift> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}