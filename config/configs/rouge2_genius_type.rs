// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rouge2GeniusType {
    #[serde(rename = "careerExp")]
    pub career_exp: i32,
    #[serde(rename = "geniusId")]
    pub genius_id: i32,
    pub order: i32,
    #[serde(rename = "pointCost")]
    pub point_cost: i32,
    pub talent: i32,
}
pub struct Rouge2GeniusTypeTable {
    records: Vec<Rouge2GeniusType>,
}

impl Rouge2GeniusTypeTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Rouge2GeniusType> = crate::load_rows(path)?;

        Ok(Self { records })
    }

    #[inline]
    pub fn all(&self) -> &[Rouge2GeniusType] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Rouge2GeniusType> {
        self.records.iter()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}
