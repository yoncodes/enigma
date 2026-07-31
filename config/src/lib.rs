// Generated modules are committed. Refresh them explicitly with `cargo run -p config_codegen`.
include!("../../config/configs/mod.rs");

pub(crate) fn load_rows<T: serde::de::DeserializeOwned>(path: &str) -> anyhow::Result<Vec<T>> {
    let file = std::fs::File::open(path)?;
    let (_, rows): (String, Vec<T>) = serde_json::from_reader(std::io::BufReader::new(file))?;
    Ok(rows)
}

// Handwritten semantic queries belong here, not in generated table files or callers.
mod activity_query;
mod battle_pass;
mod battle_query;
mod dungeon;
mod equipment;
mod hero;
mod player;
mod reward_query;
mod room;
mod scene;
mod summon_query;
mod task;
mod tower;

pub mod configs {
    pub use crate::{GameDB, get, init, try_get};
}
