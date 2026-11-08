// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekwalkType {
    #[serde(rename = "canResetLayer")]
    pub can_reset_layer: i32,
    #[serde(rename = "heroCd")]
    pub hero_cd: i32,
    #[serde(rename = "isRefresh")]
    pub is_refresh: i32,
    #[serde(rename = "showDetail")]
    pub show_detail: i32,
    pub star: i32,
    #[serde(rename = "starNum")]
    pub star_num: i32,
    #[serde(rename = "type")]
    pub r#type: i32,
}
pub struct WeekwalkTypeTable {
    records: Vec<WeekwalkType>,
}

impl WeekwalkTypeTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<WeekwalkType> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[WeekwalkType] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, WeekwalkType> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}