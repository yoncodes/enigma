//! Validated, embedded map layouts generated from the matching client data.

use std::{collections::HashSet, sync::OnceLock};

use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Deserialize)]
pub struct Layout {
    pub map_id: i32,
    pub chapter_id: i32,
    pub episode_id: i32,
    pub scene_id: i32,
    pub spawn: Spawn,
    pub nodes: Vec<Node>,
    pub areas: Vec<Area>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Spawn {
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub direction: i32,
    pub area: i32,
}

/// Client map node: x, y, height, area id, camera id.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Node(pub i32, pub i32, pub i32, pub i32, pub i32);

#[derive(Clone, Debug, Deserialize)]
pub struct Area {
    pub id: i32,
    pub resettable: bool,
    pub units: Vec<Unit>,
}

/// Map effects consist of a numeric action and a parameter of varying type.
#[derive(Clone, Debug, Deserialize)]
pub struct Effect(pub i32, pub Value);

#[derive(Clone, Debug, Deserialize)]
pub struct Unit {
    pub id: i32,
    pub kind: i32,
    pub x: i32,
    pub y: i32,
    pub offset_size: [i32; 4],
    pub direction: i32,
    pub initial_status: i32,
    pub trigger_effects: Vec<Effect>,
    pub done_effects: Vec<Effect>,
    pub special_data: Vec<Value>,
    pub trigger_by_click: bool,
    pub default_walkable: bool,
    pub counter_id: i32,
    pub is_photic: bool,
    pub can_move: bool,
    pub can_use_item: bool,
    pub trigger_dir: i32,
    pub is_strong: bool,
    pub can_trigger_gear: bool,
    pub group_id: i32,
}

impl Layout {
    pub fn node(&self, x: i32, y: i32) -> Option<&Node> {
        self.nodes.iter().find(|node| node.0 == x && node.1 == y)
    }

    pub fn unit(&self, id: i32) -> Option<&Unit> {
        self.areas.iter().flat_map(|area| &area.units).find(|unit| unit.id == id)
    }

    fn validate(&self) -> anyhow::Result<()> {
        let areas: HashSet<_> = self.areas.iter().map(|area| area.id).collect();
        anyhow::ensure!(areas.len() == self.areas.len(), "duplicate area in map {}", self.map_id);

        let mut nodes = HashSet::new();
        for node in &self.nodes {
            anyhow::ensure!(nodes.insert((node.0, node.1)), "duplicate node in map {}", self.map_id);
            anyhow::ensure!(areas.contains(&node.3), "unknown node area in map {}", self.map_id);
        }

        let mut ids = HashSet::new();
        let mut initially_bound_pots = HashSet::new();
        let mut spawn_count = 0;
        for area in &self.areas {
            for unit in &area.units {
                anyhow::ensure!(ids.insert(unit.id), "duplicate unit in map {}", self.map_id);
                anyhow::ensure!(nodes.contains(&(unit.x, unit.y)), "unit outside map {}", self.map_id);
                for effect in unit.trigger_effects.iter().chain(unit.done_effects.iter()).filter(|effect| effect.0 == 11) {
                    let target = effect.1.as_i64().and_then(|value| i32::try_from(value).ok())
                        .or_else(|| effect.1.as_str().and_then(|value| value.parse::<i32>().ok()));
                    anyhow::ensure!(target.is_some_and(|id| areas.contains(&id)),
                        "invalid area-reveal effect in map {}", self.map_id);
                }
                if unit.kind == 222 {
                    if let Some(value) = unit.special_data.first().and_then(|value| value.as_str()).filter(|value| !value.is_empty()) {
                        let pot_id = value.parse::<i32>()?;
                        let pot = self.unit(pot_id).ok_or_else(|| anyhow::anyhow!("missing initial pipe pot in map {}", self.map_id))?;
                        anyhow::ensure!(pot.kind == 224 && (pot.x, pot.y) == (unit.x, unit.y)
                            && initially_bound_pots.insert(pot_id), "invalid initial pipe pot in map {}", self.map_id);
                    }
                }
                if unit.kind == 0 {
                    spawn_count += 1;
                    anyhow::ensure!(
                        unit.id == self.spawn.id && unit.x == self.spawn.x && unit.y == self.spawn.y
                            && unit.direction == self.spawn.direction && area.id == self.spawn.area,
                        "spawn mismatch in map {}", self.map_id
                    );
                }
            }
        }
        anyhow::ensure!(spawn_count == 1, "expected one spawn in map {}", self.map_id);
        let spawn_node = self.node(self.spawn.x, self.spawn.y)
            .ok_or_else(|| anyhow::anyhow!("missing spawn node in map {}", self.map_id))?;
        anyhow::ensure!(spawn_node.3 == self.spawn.area, "spawn area mismatch in map {}", self.map_id);
        Ok(())
    }
}

static LAYOUTS: OnceLock<Result<Vec<Layout>, String>> = OnceLock::new();

/// Load and validate layouts once; failure never silently creates an empty map.
pub fn layouts() -> anyhow::Result<&'static [Layout]> {
    match LAYOUTS.get_or_init(|| {
        let layouts: Vec<Layout> = serde_json::from_str(include_str!("../data/explore_layouts.json"))
            .map_err(|error| format!("invalid exploration layouts: {error}"))?;
        let mut ids = HashSet::new();
        for layout in &layouts {
            if !ids.insert(layout.map_id) {
                return Err(format!("duplicate exploration map {}", layout.map_id));
            }
            layout.validate().map_err(|error| error.to_string())?;
        }
        Ok(layouts)
    }) {
        Ok(layouts) => Ok(layouts),
        Err(error) => Err(anyhow::anyhow!("{error}")),
    }
}

pub fn layout(map_id: i32) -> anyhow::Result<Option<&'static Layout>> {
    Ok(layouts()?.iter().find(|layout| layout.map_id == map_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_layouts_are_consistent_and_have_expected_start() {
        let all = layouts().expect("user-provided maps must load");
        assert_eq!(all.len(), 9);
        let first = layout(101).unwrap().unwrap();
        assert_eq!((first.chapter_id, first.episode_id, first.scene_id), (1401, 140101, 70101));
        assert_eq!((first.spawn.id, first.spawn.x, first.spawn.y, first.spawn.area), (169, 1, 6, 1));
        assert_eq!((first.node(1, 6).unwrap().2, first.node(1, 6).unwrap().3), (0, 1));
        let door = first.unit(189).unwrap();
        assert_eq!((door.kind, door.x, door.y, door.initial_status), (202, 13, 2, 3));
        assert!(!door.default_walkable);
        assert!(layout(999999).unwrap().is_none());
    }

    #[test]
    fn inconsistent_layout_is_rejected() {
        let mut layout = layout(101).unwrap().unwrap().clone();
        layout.nodes.push(layout.nodes[0]);
        assert!(layout.validate().is_err(), "duplicate nodes must be rejected");
        layout.nodes.pop();
        layout.spawn.x += 1;
        assert!(layout.validate().is_err(), "a changed spawn must be rejected");
    }
}
