// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcadeConst {
    pub id: i32,
    pub mlvalue: String,
    pub value: String,
}
use std::collections::HashMap;

pub struct ArcadeConstTable {
    records: Vec<ArcadeConst>,
    by_id: HashMap<i32, usize>,
}

impl ArcadeConstTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<ArcadeConst> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&ArcadeConst> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[ArcadeConst] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, ArcadeConst> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}