// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekwalkVer2Time {
    #[serde(rename = "issueId")]
    pub issue_id: i32,
    #[serde(rename = "optionalSkills")]
    pub optional_skills: String,
    #[serde(rename = "ruleFront")]
    pub rule_front: String,
    #[serde(rename = "ruleIcon")]
    pub rule_icon: String,
    #[serde(rename = "ruleRear")]
    pub rule_rear: String,
    #[serde(rename = "timeId")]
    pub time_id: i32,
}
pub struct WeekwalkVer2TimeTable {
    records: Vec<WeekwalkVer2Time>,
}

impl WeekwalkVer2TimeTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<WeekwalkVer2Time> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[WeekwalkVer2Time] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, WeekwalkVer2Time> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}