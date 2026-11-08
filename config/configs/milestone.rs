// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    #[serde(rename = "activityId")]
    pub activity_id: i32,
    #[serde(rename = "isNotPush")]
    pub is_not_push: bool,
    #[serde(rename = "milestoneId")]
    pub milestone_id: i32,
    #[serde(rename = "type")]
    pub r#type: i32,
}
pub struct MilestoneTable {
    records: Vec<Milestone>,
}

impl MilestoneTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Milestone> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[Milestone] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Milestone> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}