// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rouge2Attribute {
    #[serde(rename = "careerDesc")]
    pub career_desc: String,
    #[serde(rename = "exAttrLimit")]
    pub ex_attr_limit: String,
    pub icon: String,
    pub id: i32,
    pub level: String,
    pub min: i32,
    pub name: String,
    #[serde(rename = "showMax")]
    pub show_max: i32,
    #[serde(rename = "spriteIndex")]
    pub sprite_index: i32,
    #[serde(rename = "type")]
    pub r#type: i32,
}
use std::collections::HashMap;

pub struct Rouge2AttributeTable {
    records: Vec<Rouge2Attribute>,
    by_id: HashMap<i32, usize>,
}

impl Rouge2AttributeTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Rouge2Attribute> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&Rouge2Attribute> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[Rouge2Attribute] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Rouge2Attribute> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}