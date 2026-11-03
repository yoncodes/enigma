// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Weekwalk {
    #[serde(rename = "fightIdFront")]
    pub fight_id_front: i32,
    #[serde(rename = "fightIdRear")]
    pub fight_id_rear: i32,
    pub id: i32,
    #[serde(rename = "issueId")]
    pub issue_id: i32,
    pub layer: i32,
    #[serde(rename = "notCdHeroCount")]
    pub not_cd_hero_count: i32,
    #[serde(rename = "preId")]
    pub pre_id: i32,
    #[serde(rename = "resIdFront")]
    pub res_id_front: i32,
    #[serde(rename = "resIdRear")]
    pub res_id_rear: i32,
    #[serde(rename = "sceneId")]
    pub scene_id: i32,
    #[serde(rename = "type")]
    pub r#type: i32,
}
use std::collections::HashMap;

pub struct WeekwalkTable {
    records: Vec<Weekwalk>,
    by_id: HashMap<i32, usize>,
}

impl WeekwalkTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Weekwalk> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&Weekwalk> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[Weekwalk] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Weekwalk> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}