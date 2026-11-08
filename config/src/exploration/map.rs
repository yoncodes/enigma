use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub x: i32,
    pub y: i32,
    pub height: i32,
    pub area_id: i32,
    pub camera_id: i32,
}

#[derive(Debug, Deserialize)]
pub struct Effect(pub i32, pub Value);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Unit {
    pub id: i32,
    #[serde(rename = "type")]
    pub kind: i32,
    pub node_pos: [i32; 2],
    pub area_id: i32,
    pub interact: i32,
    pub unit_dir: i32,
    pub trigger_dir: i32,
    pub offset_size: [i32; 4],
    pub default_walkable: bool,
    pub trigger_by_click: bool,
    pub trigger_effects: Vec<Effect>,
    pub done_effects: Vec<Effect>,
    pub special_datas: Vec<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Area {
    pub id: i32,
    pub can_reset: bool,
    pub units: Vec<Unit>,
}

#[derive(Deserialize)]
pub(super) struct MapData {
    pub id: i32,
    pub nodes: Vec<Node>,
    pub areas: Vec<Area>,
}

#[derive(Debug)]
pub struct Map {
    pub id: i32,
    pub nodes: BTreeMap<(i32, i32), Node>,
    pub areas: BTreeMap<i32, Area>,
}

impl Map {
    pub(super) fn read(data: MapData) -> Result<Self> {
        let mut map = Self {
            id: data.id,
            nodes: BTreeMap::new(),
            areas: BTreeMap::new(),
        };
        for node in data.nodes {
            ensure!(
                map.nodes.insert((node.x, node.y), node).is_none(),
                "duplicate exploration node"
            );
        }
        let mut unit_ids = BTreeSet::new();
        for area in data.areas {
            for unit in &area.units {
                ensure!(unit_ids.insert(unit.id), "duplicate exploration unit");
                ensure!(
                    unit.area_id == area.id,
                    "exploration unit belongs to another area"
                );
                ensure!(
                    map.nodes
                        .contains_key(&(unit.node_pos[0], unit.node_pos[1])),
                    "exploration unit is outside the map"
                );
            }
            ensure!(
                map.areas.insert(area.id, area).is_none(),
                "duplicate exploration area"
            );
        }
        ensure!(
            map.nodes
                .values()
                .all(|node| map.areas.contains_key(&node.area_id)),
            "exploration node has an unknown area"
        );
        Ok(map)
    }
}
