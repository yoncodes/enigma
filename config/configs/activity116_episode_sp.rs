// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity116EpisodeSp {
    pub desc: String,
    #[serde(rename = "endShow")]
    pub end_show: String,
    pub id: i32,
    #[serde(rename = "refreshDay")]
    pub refresh_day: i32,
    pub title: String,
}
use std::collections::HashMap;

pub struct Activity116EpisodeSpTable {
    records: Vec<Activity116EpisodeSp>,
    by_id: HashMap<i32, usize>,
}

impl Activity116EpisodeSpTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity116EpisodeSp> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&Activity116EpisodeSp> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[Activity116EpisodeSp] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity116EpisodeSp> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}