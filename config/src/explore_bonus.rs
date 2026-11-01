//! Dialogue choice counts extracted from the supplied client configuration.
use std::{collections::HashSet, sync::OnceLock};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct BonusScene {
    pub id: i32,
    pub choices_per_step: Vec<usize>,
}

static SCENES: OnceLock<Result<Vec<BonusScene>, String>> = OnceLock::new();

pub fn scene(id: i32) -> anyhow::Result<Option<&'static BonusScene>> {
    let scenes = SCENES.get_or_init(|| {
        let data: Vec<BonusScene> = serde_json::from_str(include_str!("../data/explore_bonus_scenes.json"))
            .map_err(|e| e.to_string())?;
        let mut ids = HashSet::new();
        if data.iter().any(|scene| !ids.insert(scene.id) || scene.choices_per_step.is_empty()
            || scene.choices_per_step.iter().any(|&choices| choices > 32)) {
            return Err("invalid exploration bonus-scene configuration".into());
        }
        Ok(data)
    }).as_ref().map_err(|error| anyhow::anyhow!("{error}"))?;
    Ok(scenes.iter().find(|scene| scene.id == id))
}
