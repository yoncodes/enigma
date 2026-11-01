// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity128Gallery {
    #[serde(rename = "galleryBg")]
    pub gallery_bg: String,
    pub id: i32,
    #[serde(rename = "lurkNumber")]
    pub lurk_number: i32,
    #[serde(rename = "minType")]
    pub min_type: i32,
    pub offline: i32,
    pub point: String,
    #[serde(rename = "pointExp")]
    pub point_exp: String,
    #[serde(rename = "recommendStrategy")]
    pub recommend_strategy: String,
    pub sort: i32,
    #[serde(rename = "type")]
    pub r#type: i32,
    #[serde(rename = "typeName")]
    pub type_name: String,
}
use std::collections::HashMap;

pub struct Activity128GalleryTable {
    records: Vec<Activity128Gallery>,
    by_id: HashMap<i32, usize>,
}

impl Activity128GalleryTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity128Gallery> = crate::load_rows(path)?;

        let mut by_id = HashMap::with_capacity(records.len());

        for (idx, record) in records.iter().enumerate() {
            by_id.insert(record.id, idx);
        }

        Ok(Self {
            records,
            by_id,
        })
    }

    #[inline]
    pub fn get(&self, id: i32) -> Option<&Activity128Gallery> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[Activity128Gallery] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity128Gallery> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}