// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcadeCollection {
    pub category: String,
    pub describe: String,
    #[serde(rename = "dropWeight")]
    pub drop_weight: i32,
    pub durable: i32,
    #[serde(rename = "goodsWeight")]
    pub goods_weight: i32,
    pub icon: String,
    pub id: i32,
    #[serde(rename = "isUnique")]
    pub is_unique: bool,
    pub level: i32,
    pub name: String,
    #[serde(rename = "passiveSkills")]
    pub passive_skills: String,
    #[serde(rename = "posOffset")]
    pub pos_offset: Option<serde_json::Value>,
    pub price: i32,
    #[serde(rename = "resPath")]
    pub res_path: String,
    pub scale: Vec<f32>,
    #[serde(rename = "showTmpAttrChange")]
    pub show_tmp_attr_change: i32,
    #[serde(rename = "type")]
    pub r#type: String,
}
use std::collections::HashMap;

pub struct ArcadeCollectionTable {
    records: Vec<ArcadeCollection>,
    by_id: HashMap<i32, usize>,
}

impl ArcadeCollectionTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<ArcadeCollection> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&ArcadeCollection> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[ArcadeCollection] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, ArcadeCollection> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}