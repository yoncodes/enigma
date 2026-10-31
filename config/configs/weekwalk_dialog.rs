// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekwalkDialog {
    pub content: String,
    pub id: i32,
    pub option_param: String,
    pub param: String,
    pub single: i32,
    pub speaker: String,
    #[serde(rename = "stepId")]
    pub step_id: i32,
    #[serde(rename = "type")]
    pub r#type: String,
}
use std::collections::HashMap;

pub struct WeekwalkDialogTable {
    records: Vec<WeekwalkDialog>,
    by_group: HashMap<i32, Vec<usize>>,
}

impl WeekwalkDialogTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<WeekwalkDialog> = crate::load_rows(path)?;

        let mut by_group: HashMap<i32, Vec<usize>> = HashMap::new();

        for (idx, record) in records.iter().enumerate() {
            by_group.entry(record.id).or_default().push(idx);
        }

        Ok(Self { records, by_group })
    }

    pub fn by_group(&self, group_id: i32) -> impl Iterator<Item = &'_ WeekwalkDialog> + '_ {
        self.by_group
            .get(&group_id)
            .into_iter()
            .flat_map(|idxs| idxs.iter())
            .map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[WeekwalkDialog] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, WeekwalkDialog> {
        self.records.iter()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}
