//! Immutable exploration geometry and unit data from the matching client files.

use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context, Result, ensure};

mod fields;
mod map;
mod source;

pub use map::{Area, Effect, Map, Node, Unit};

pub struct MapTable {
    maps: BTreeMap<i32, Map>,
}

impl MapTable {
    pub fn load(data_dir: &str, scenes: &crate::explore_scene::ExploreSceneTable) -> Result<Self> {
        let root = Path::new(data_dir)
            .parent()
            .context("game data root is missing")?
            .join("lua/modules");
        let definitions = root.join("logic/explore/controller/ExploreUnitMoFieldEnum.lua");
        let fields = fields::Fields::read(
            &fs::read_to_string(&definitions)
                .with_context(|| format!("read {}", definitions.display()))?,
        )?;
        let mut maps = BTreeMap::new();
        for scene in scenes.iter() {
            let path = root.join(format!("configs/explore/lua_explore_map_{}.lua", scene.id));
            let source =
                fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
            let map = Map::read(scene.id, &source, &fields)
                .with_context(|| format!("decode {}", path.display()))?;
            ensure!(
                maps.insert(scene.id, map).is_none(),
                "duplicate exploration scene"
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
