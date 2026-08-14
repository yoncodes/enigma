// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeachingEpisode {
    #[serde(rename = "battleTasks")]
    pub battle_tasks: String,
    pub detail: String,
    pub id: i32,
    #[serde(rename = "preEpisode")]
    pub pre_episode: i32,
    #[serde(rename = "taskDetail")]
    pub task_detail: String,
    pub teaching: i32,
    #[serde(rename = "type")]
    pub r#type: i32,
}
use std::collections::HashMap;

pub struct TeachingEpisodeTable {
    records: Vec<TeachingEpisode>,
    by_id: HashMap<i32, usize>,
}

impl TeachingEpisodeTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<TeachingEpisode> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&TeachingEpisode> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[TeachingEpisode] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, TeachingEpisode> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}