// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnbackTask {
    #[serde(rename = "acceptNeedOnlineSeconds")]
    pub accept_need_online_seconds: i32,
    pub bonus: String,
    pub desc: String,
    pub id: i32,
    #[serde(rename = "isOnline")]
    pub is_online: i32,
    #[serde(rename = "isOnlineTimeTask")]
    pub is_online_time_task: bool,
    #[serde(rename = "jumpId")]
    pub jump_id: i32,
    #[serde(rename = "jumpUi")]
    pub jump_ui: String,
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
    #[serde(rename = "randomPriority")]
    pub random_priority: i32,
    #[serde(rename = "showDay")]
    pub show_day: i32,
    #[serde(rename = "sortId")]
    pub sort_id: i32,
    #[serde(rename = "turnbackId")]
    pub turnback_id: i32,
    #[serde(rename = "turnbackTaskType")]
    pub turnback_task_type: i32,
    #[serde(rename = "type")]
    pub r#type: i32,
    #[serde(rename = "unlockDay")]
    pub unlock_day: i32,
}
use std::collections::HashMap;

pub struct TurnbackTaskTable {
    records: Vec<TurnbackTask>,
    by_id: HashMap<i32, usize>,
}

impl TurnbackTaskTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<TurnbackTask> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&TurnbackTask> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[TurnbackTask] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, TurnbackTask> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}