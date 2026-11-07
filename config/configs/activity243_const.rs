// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity243Const {
    #[serde(rename = "activityId")]
    pub activity_id: i32,
    pub id: i32,
    pub value: String,
    pub value2: String,
}
pub struct Activity243ConstTable {
    records: Vec<Activity243Const>,
}

impl Activity243ConstTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity243Const> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[Activity243Const] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity243Const> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}