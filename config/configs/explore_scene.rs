// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreScene {
    #[serde(rename = "chapterId")]
    pub chapter_id: i32,
    #[serde(rename = "episodeId")]
    pub episode_id: i32,
    pub id: i32,
    #[serde(rename = "sceneId")]
    pub scene_id: i32,
    #[serde(rename = "signsId")]
    pub signs_id: String,
}
use std::collections::HashMap;

pub struct ExploreSceneTable {
    records: Vec<ExploreScene>,
    by_id: HashMap<i32, usize>,
}

impl ExploreSceneTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<ExploreScene> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&ExploreScene> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[ExploreScene] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, ExploreScene> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}