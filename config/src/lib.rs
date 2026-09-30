// Generated modules are committed. Refresh them explicitly with `cargo run -p config_codegen`.
include!("../../config/configs/mod.rs");

pub(crate) fn load_rows<T: serde::de::DeserializeOwned>(path: &str) -> anyhow::Result<Vec<T>> {
    let file = std::fs::File::open(path)?;
    let (_, rows): (String, Vec<T>) = serde_json::from_reader(std::io::BufReader::new(file))?;
    Ok(rows)
}

/// Loads the workspace game-data snapshot once for config tests.
#[cfg(test)]
pub(crate) fn init_test_config() {
    let data_dir = std::env::var_os("ENIGMA_BATTLE_DATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .expect("config crate must live under the workspace root")
                .join("data/excel2json")
        });
    init(
        data_dir
            .to_str()
            .expect("workspace game-data path must be valid UTF-8"),
    )
    .expect("test game data must load");
}

// Handwritten semantic queries belong here, not in generated table files or callers.
mod activity_query;
mod battle_pass;
mod dungeon;
mod equipment;
mod hero;
mod player;
mod reward_query;
mod room;
mod scene;
mod summon_query;
mod task;
mod toughness;
mod tower;

pub mod configs {
    pub use crate::{GameDB, get, init, try_get};
}
