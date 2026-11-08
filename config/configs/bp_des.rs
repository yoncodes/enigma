// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BpDes {
    #[serde(rename = "bpId")]
    pub bp_id: i32,
    pub des: String,
    pub icon: String,
    #[serde(rename = "iconDesc")]
    pub icon_desc: String,
    #[serde(rename = "iconType")]
    pub icon_type: i32,
    pub id: i32,
    pub items: String,
    #[serde(rename = "tagTxt")]
    pub tag_txt: String,
    #[serde(rename = "tagType")]
    pub tag_type: i32,
    #[serde(rename = "type")]
    pub r#type: i32,
}
use std::collections::HashMap;

pub struct BpDesTable {
    records: Vec<BpDes>,
    by_id: HashMap<i32, usize>,
}

impl BpDesTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<BpDes> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&BpDes> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[BpDes] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, BpDes> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}