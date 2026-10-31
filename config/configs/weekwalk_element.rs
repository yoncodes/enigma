// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekwalkElement {
    #[serde(rename = "bonusGroup")]
    pub bonus_group: String,
    pub desc: String,
    #[serde(rename = "disappearEffect")]
    pub disappear_effect: String,
    pub effect: String,
    #[serde(rename = "generalType")]
    pub general_type: i32,
    pub id: i32,
    #[serde(rename = "isBoss")]
    pub is_boss: i32,
    #[serde(rename = "lightOffsetPos")]
    pub light_offset_pos: String,
    pub param: String,
    pub pos: String,
    pub res: String,
    #[serde(rename = "roundId")]
    pub round_id: i32,
    #[serde(rename = "skipFinish")]
    pub skip_finish: i32,
    #[serde(rename = "smokeMaskOffset")]
    pub smoke_mask_offset: String,
    #[serde(rename = "starOffsetPos")]
    pub star_offset_pos: String,
    #[serde(rename = "tipOffsetPos")]
    pub tip_offset_pos: String,
    #[serde(rename = "type")]
    pub r#type: String,
}
use std::collections::HashMap;

pub struct WeekwalkElementTable {
    records: Vec<WeekwalkElement>,
    by_id: HashMap<i32, usize>,
}

impl WeekwalkElementTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<WeekwalkElement> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&WeekwalkElement> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[WeekwalkElement] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, WeekwalkElement> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}