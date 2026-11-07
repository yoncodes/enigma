// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekwalkVer2Cup {
    #[serde(rename = "cupNo")]
    pub cup_no: i32,
    #[serde(rename = "cupTask")]
    pub cup_task: String,
    pub desc: String,
    pub desc1: String,
    pub desc2: String,
    pub desc3: String,
    #[serde(rename = "fightType")]
    pub fight_type: i32,
    pub id: i32,
    #[serde(rename = "layerId")]
    pub layer_id: i32,
    #[serde(rename = "paramOfProgressDesc")]
    pub param_of_progress_desc: String,
    #[serde(rename = "progressDesc")]
    pub progress_desc: String,
}
use std::collections::HashMap;

pub struct WeekwalkVer2CupTable {
    records: Vec<WeekwalkVer2Cup>,
    by_id: HashMap<i32, usize>,
}

impl WeekwalkVer2CupTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<WeekwalkVer2Cup> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&WeekwalkVer2Cup> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[WeekwalkVer2Cup] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, WeekwalkVer2Cup> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}