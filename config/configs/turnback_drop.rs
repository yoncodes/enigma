// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnbackDrop {
    pub id: i32,
    #[serde(rename = "jumpId")]
    pub jump_id: i32,
    pub level: i32,
    #[serde(rename = "listenerParam")]
    pub listener_param: String,
    pub name: String,
    #[serde(rename = "picPath")]
    pub pic_path: String,
    #[serde(rename = "showTxt")]
    pub show_txt: String,
    #[serde(rename = "type")]
    pub r#type: i32,
}
use std::collections::HashMap;

pub struct TurnbackDropTable {
    records: Vec<TurnbackDrop>,
    by_id: HashMap<i32, usize>,
}

impl TurnbackDropTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<TurnbackDrop> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&TurnbackDrop> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[TurnbackDrop] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, TurnbackDrop> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}