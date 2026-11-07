// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity128ExpandBond {
    #[serde(rename = "activeNum")]
    pub active_num: i32,
    #[serde(rename = "activityId")]
    pub activity_id: i32,
    #[serde(rename = "backingIcon")]
    pub backing_icon: String,
    pub desc: String,
    pub effects: String,
    #[serde(rename = "groupId")]
    pub group_id: i32,
    pub icon: String,
    pub id: i32,
    pub level: i32,
    pub name: String,
    #[serde(rename = "numberIcon")]
    pub number_icon: String,
    pub tag1: String,
    pub tag2: String,
    #[serde(rename = "tagType")]
    pub tag_type: i32,
}
use std::collections::HashMap;

pub struct Activity128ExpandBondTable {
    records: Vec<Activity128ExpandBond>,
    by_id: HashMap<i32, usize>,
    by_group: HashMap<i32, Vec<usize>>,
}

impl Activity128ExpandBondTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity128ExpandBond> = crate::load_rows(path)?;

        let mut by_id = HashMap::with_capacity(records.len());
        let mut by_group: HashMap<i32, Vec<usize>> = HashMap::new();

        for (idx, record) in records.iter().enumerate() {
            by_id.insert(record.id, idx);
            by_group.entry(record.group_id).or_default().push(idx);
        }

        Ok(Self {
            records,
            by_id,
            by_group,
        })
    }

    #[inline]
    pub fn get(&self, id: i32) -> Option<&Activity128ExpandBond> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    pub fn by_group(&self, group_id: i32) -> impl Iterator<Item = &'_ Activity128ExpandBond> + '_ {
        self.by_group
            .get(&group_id)
            .into_iter()
            .flat_map(|idxs| idxs.iter())
            .map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[Activity128ExpandBond] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity128ExpandBond> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}