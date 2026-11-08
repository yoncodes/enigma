// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnbackReturnRange {
    #[serde(rename = "lossDays")]
    pub loss_days: String,
    #[serde(rename = "rangeId")]
    pub range_id: i32,
}
pub struct TurnbackReturnRangeTable {
    records: Vec<TurnbackReturnRange>,
}

impl TurnbackReturnRangeTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<TurnbackReturnRange> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[TurnbackReturnRange] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, TurnbackReturnRange> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}