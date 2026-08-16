// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcadeAttribute {
    #[serde(rename = "currencyIcon")]
    pub currency_icon: String,
    pub extend: i32,
    pub icon: String,
    pub id: i32,
    #[serde(rename = "initVal")]
    pub init_val: i32,
    pub max: i32,
    pub min: i32,
    pub name: String,
}
use std::collections::HashMap;

pub struct ArcadeAttributeTable {
    records: Vec<ArcadeAttribute>,
    by_id: HashMap<i32, usize>,
}

impl ArcadeAttributeTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<ArcadeAttribute> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&ArcadeAttribute> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[ArcadeAttribute] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, ArcadeAttribute> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}