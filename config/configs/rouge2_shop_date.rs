// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rouge2ShopDate {
    #[serde(rename = "currencyMax")]
    pub currency_max: i32,
    #[serde(rename = "endTime")]
    pub end_time: String,
    pub id: i32,
    #[serde(rename = "shopTitle")]
    pub shop_title: String,
    #[serde(rename = "startTime")]
    pub start_time: String,
}
use std::collections::HashMap;

pub struct Rouge2ShopDateTable {
    records: Vec<Rouge2ShopDate>,
    by_id: HashMap<i32, usize>,
}

impl Rouge2ShopDateTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Rouge2ShopDate> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&Rouge2ShopDate> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[Rouge2ShopDate] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Rouge2ShopDate> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}