// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity233Task {
    #[serde(rename = "activityId")]
    pub activity_id: i32,
    #[serde(rename = "bonusMail")]
    pub bonus_mail: i32,
    #[serde(rename = "bonusScore")]
    pub bonus_score: i32,
    #[serde(rename = "bpId")]
    pub bp_id: i32,
    pub desc: String,
    #[serde(rename = "endTime")]
    pub end_time: String,
    pub id: i32,
    #[serde(rename = "isOnline")]
    pub is_online: i32,
    #[serde(rename = "jumpId")]
    pub jump_id: i32,
    #[serde(rename = "listenerParam")]
    pub listener_param: String,
    #[serde(rename = "listenerType")]
    pub listener_type: String,
    #[serde(rename = "loopType")]
    pub loop_type: i32,
    #[serde(rename = "maxProgress")]
    pub max_progress: i32,
    #[serde(rename = "minType")]
    pub min_type: String,
    pub name: String,
    #[serde(rename = "openLimit")]
    pub open_limit: String,
    pub params: String,
    pub prepose: String,
    #[serde(rename = "sortId")]
    pub sort_id: i32,
    #[serde(rename = "startTime")]
    pub start_time: String,
}
use std::collections::HashMap;

pub struct Activity233TaskTable {
    records: Vec<Activity233Task>,
    by_id: HashMap<i32, usize>,
}

impl Activity233TaskTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity233Task> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&Activity233Task> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[Activity233Task] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity233Task> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}