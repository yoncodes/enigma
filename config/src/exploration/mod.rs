//! Immutable exploration geometry and unit data from the exported JSON tables.

use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

mod map;

pub use map::{Area, Effect, Map, Node, Unit};

pub struct MapTable {
    maps: BTreeMap<i32, Map>,
}

impl MapTable {
    pub fn load(data_dir: &str, scenes: &crate::explore_scene::ExploreSceneTable) -> Result<Self> {
        let path = format!("{data_dir}/explore_map.json");
        let rows: Vec<map::MapData> =
            crate::load_rows(&path).with_context(|| format!("read {path}"))?;
        let mut maps = BTreeMap::new();
        for row in rows {
            let id = row.id;
            ensure!(scenes.get(id).is_some(), "unknown exploration scene {id}");
            let map = Map::read(row).with_context(|| format!("decode exploration map {id}"))?;
            ensure!(
                maps.insert(id, map).is_none(),
                "duplicate exploration scene"
            );
        }
        for scene in scenes.iter() {
            ensure!(
                maps.contains_key(&scene.id),
                "missing exploration map {}",
                scene.id
            );
        }
        Ok(Self { maps })
    }

    pub fn get(&self, id: i32) -> Option<&Map> {
        self.maps.get(&id)
    }
}

#[cfg(test)]
mod tests;
