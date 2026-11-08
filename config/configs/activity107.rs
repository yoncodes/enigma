// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity107 {
    #[serde(rename = "activityId")]
    pub activity_id: i32,
    #[serde(rename = "bigImg")]
    pub big_img: String,
    pub cost: String,
    pub group: i32,
    pub id: i32,
    #[serde(rename = "isOnline")]
    pub is_online: bool,
    #[serde(rename = "isShowBtnDetail")]
    pub is_show_btn_detail: bool,
    #[serde(rename = "maxBuyCount")]
    pub max_buy_count: i32,
    pub name: String,
    #[serde(rename = "offTag")]
    pub off_tag: String,
    pub order: i32,
    #[serde(rename = "originalCost")]
    pub original_cost: i32,
    #[serde(rename = "preGoodsId")]
    pub pre_goods_id: i32,
    pub product: String,
    #[serde(rename = "specProduct")]
    pub spec_product: i32,
    pub tag: i32,
}
use std::collections::HashMap;

pub struct Activity107Table {
    records: Vec<Activity107>,
    by_id: HashMap<i32, usize>,
}

impl Activity107Table {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity107> = crate::load_rows(path)?;

        let mut by_id = HashMap::with_capacity(records.len());

        for (idx, record) in records.iter().enumerate() {
            by_id.insert(record.id, idx);
        }

        Ok(Self { records, by_id })
    }

    #[inline]
    pub fn get(&self, id: i32) -> Option<&Activity107> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[Activity107] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity107> {
        self.records.iter()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}
