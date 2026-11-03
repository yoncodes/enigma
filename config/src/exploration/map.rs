use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::Value;

use super::fields::Fields;

#[derive(Debug, Deserialize)]
#[serde(from = "[i32; 5]")]
pub struct Node {
    pub x: i32,
    pub y: i32,
    pub height: i32,
    pub area_id: i32,
    pub camera_id: i32,
}

impl From<[i32; 5]> for Node {
    fn from([x, y, height, area_id, camera_id]: [i32; 5]) -> Self {
        Self {
            x,
            y,
            height,
            area_id,
            camera_id,
        }
    }
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
    pub offset_size: [i32; 4],
    pub default_walkable: bool,
    pub trigger_by_click: bool,
    pub trigger_effects: Vec<Effect>,
    pub done_effects: Vec<Effect>,
    pub special_datas: Vec<Value>,
}

#[derive(Debug)]
pub struct Area {
    pub id: i32,
    pub can_reset: bool,
    pub units: Vec<Unit>,
}

#[derive(Debug)]
pub struct Map {
    pub id: i32,
    pub nodes: BTreeMap<(i32, i32), Node>,
    pub areas: BTreeMap<i32, Area>,
}

impl Map {
    pub(super) fn read(id: i32, source: &str, fields: &Fields) -> Result<Self> {
        let mut rows = super::source::read(source)?.into_iter();
        let nodes: Vec<Node> =
            serde_json::from_value(rows.next().context("map nodes are missing")?)?;
        let mut map = Self {
            id,
            nodes: BTreeMap::new(),
            areas: BTreeMap::new(),
        };
        for node in nodes {
            ensure!(
                map.nodes.insert((node.x, node.y), node).is_none(),
                "duplicate exploration node"
            );
        }
        let mut unit_ids = BTreeSet::new();
        for row in rows {
            let (area_id, units, can_reset): (i32, Vec<Vec<Value>>, bool) =
                serde_json::from_value(row)?;
            let mut area = Area {
                id: area_id,
                can_reset,
                units: Vec::new(),
            };
            for row in units {
                let unit = fields.unit(row)?;
                ensure!(unit_ids.insert(unit.id), "duplicate exploration unit");
                ensure!(
                    unit.area_id == area_id,
                    "exploration unit belongs to another area"
                );
                ensure!(
                    map.nodes
                        .contains_key(&(unit.node_pos[0], unit.node_pos[1])),
                    "exploration unit is outside the map"
                );
                area.units.push(unit);
            }
            ensure!(
                map.areas.insert(area_id, area).is_none(),
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
