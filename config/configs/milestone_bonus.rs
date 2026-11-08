// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MilestoneBonus {
    pub bonus: String,
    #[serde(rename = "bonusId")]
    pub bonus_id: i32,
    #[serde(rename = "milestoneId")]
    pub milestone_id: i32,
    #[serde(rename = "needProgress")]
    pub need_progress: i32,
    pub special: bool,
}
pub struct MilestoneBonusTable {
    records: Vec<MilestoneBonus>,
}

impl MilestoneBonusTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<MilestoneBonus> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[MilestoneBonus] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, MilestoneBonus> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}