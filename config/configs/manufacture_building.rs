// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManufactureBuilding {
    #[serde(rename = "buildIcon")]
    pub build_icon: String,
    #[serde(rename = "cameraIds")]
    pub camera_ids: String,
    pub id: i32,
    #[serde(rename = "placeCost")]
    pub place_cost: String,
    #[serde(rename = "placeNoCost")]
    pub place_no_cost: i32,
    #[serde(rename = "placeTradeLevel")]
    pub place_trade_level: i32,
    #[serde(rename = "taskIcon")]
    pub task_icon: String,
    #[serde(rename = "tradeGroupId")]
    pub trade_group_id: i32,
    #[serde(rename = "upgradeGroupId")]
    pub upgrade_group_id: i32,
}
use std::collections::HashMap;

pub struct ManufactureBuildingTable {
    records: Vec<ManufactureBuilding>,
    by_id: HashMap<i32, usize>,
    by_group: HashMap<i32, Vec<usize>>,
}

impl ManufactureBuildingTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<ManufactureBuilding> = crate::load_rows(path)?;

        let mut by_id = HashMap::with_capacity(records.len());
        let mut by_group: HashMap<i32, Vec<usize>> = HashMap::new();

        for (idx, record) in records.iter().enumerate() {
            by_id.insert(record.id, idx);
            by_group.entry(record.trade_group_id).or_default().push(idx);
        }

        Ok(Self {
            records,
            by_id,
            by_group,
        })
    }

    #[inline]
    pub fn get(&self, id: i32) -> Option<&ManufactureBuilding> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    pub fn by_group(&self, group_id: i32) -> impl Iterator<Item = &'_ ManufactureBuilding> + '_ {
        self.by_group
            .get(&group_id)
            .into_iter()
            .flat_map(|idxs| idxs.iter())
            .map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[ManufactureBuilding] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, ManufactureBuilding> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}
