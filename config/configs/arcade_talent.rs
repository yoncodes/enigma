// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcadeTalent {
    pub cost: i32,
    #[serde(rename = "displayType")]
    pub display_type: String,
    pub id: i32,
    pub level: i32,
    pub name: String,
    pub skill: String,
}
use std::collections::HashMap;

pub struct ArcadeTalentTable {
    records: Vec<ArcadeTalent>,
    by_id: HashMap<i32, usize>,
}

impl ArcadeTalentTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<ArcadeTalent> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&ArcadeTalent> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[ArcadeTalent] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, ArcadeTalent> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}