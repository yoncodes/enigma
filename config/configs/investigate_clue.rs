// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestigateClue {
    #[serde(rename = "defaultUnlock")]
    pub default_unlock: i32,
    #[serde(rename = "detailedDesc")]
    pub detailed_desc: String,
    pub id: i32,
    #[serde(rename = "infoID")]
    pub info_id: i32,
    #[serde(rename = "mapElement")]
    pub map_element: i32,
    #[serde(rename = "mapRes")]
    pub map_res: String,
    #[serde(rename = "mapResLocked")]
    pub map_res_locked: String,
    #[serde(rename = "relatedDesc")]
    pub related_desc: String,
    pub res: String,
}
use std::collections::HashMap;

pub struct InvestigateClueTable {
    records: Vec<InvestigateClue>,
    by_id: HashMap<i32, usize>,
}

impl InvestigateClueTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<InvestigateClue> = crate::load_rows(path)?;

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
    pub fn get(&self, id: i32) -> Option<&InvestigateClue> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[InvestigateClue] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, InvestigateClue> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}
