// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekwalkVer2Skill {
    pub desc: String,
    pub icon: String,
    pub id: i32,
    pub name: String,
    pub rules: String,
}
use std::collections::HashMap;

pub struct WeekwalkVer2SkillTable {
    records: Vec<WeekwalkVer2Skill>,
    by_id: HashMap<i32, usize>,
}

impl WeekwalkVer2SkillTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<WeekwalkVer2Skill> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&WeekwalkVer2Skill> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[WeekwalkVer2Skill] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, WeekwalkVer2Skill> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}