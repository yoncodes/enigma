// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity229Skill {
    pub desc: String,
    pub icon: String,
    pub id: i32,
    pub name: String,
    pub rules: String,
    #[serde(rename = "rulesTarget")]
    pub rules_target: i32,
}
use std::collections::HashMap;

pub struct Activity229SkillTable {
    records: Vec<Activity229Skill>,
    by_id: HashMap<i32, usize>,
}

impl Activity229SkillTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity229Skill> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&Activity229Skill> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[Activity229Skill] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity229Skill> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}