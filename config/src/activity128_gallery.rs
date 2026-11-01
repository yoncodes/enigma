//! Boss types and score thresholds from the matching client configuration.
use serde::Deserialize;
use std::{collections::HashSet, sync::OnceLock};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage {
    pub activity_id: i32,
    pub stage: i32,
    #[serde(rename = "type")]
    pub boss_type: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gallery {
    #[serde(rename = "type")]
    pub boss_type: i32,
    pub point: Vec<i32>,
    pub point_exp: Vec<i32>,
}

#[derive(Debug, Deserialize)]
pub struct BossData {
    pub stages: Vec<Stage>,
    pub gallery: Vec<Gallery>,
}

static DATA: OnceLock<Result<BossData, String>> = OnceLock::new();

pub fn data() -> anyhow::Result<&'static BossData> {
    match DATA.get_or_init(|| {
        let rows: BossData = serde_json::from_str(include_str!("../data/activity128_gallery.json"))
            .map_err(|error| error.to_string())?;
        let mut stages = HashSet::new();
        let mut types = HashSet::new();
        if rows.stages.is_empty() || rows.gallery.is_empty()
            || rows.stages.iter().any(|r| r.activity_id <= 0 || r.stage <= 0 || r.boss_type <= 0
                || !stages.insert((r.activity_id, r.stage)))
            || rows.gallery.iter().any(|r| r.boss_type <= 0 || !types.insert(r.boss_type)
                || r.point.is_empty() || r.point.len() != r.point_exp.len()
                || r.point[0] <= 0 || r.point.windows(2).any(|w| w[0] >= w[1])
                || r.point_exp.iter().any(|v| *v < 0)) {
            return Err("invalid or duplicate boss-gallery configuration".to_owned());
        }
        Ok(rows)
    }) {
        Ok(data) => Ok(data),
        Err(error) => Err(anyhow::anyhow!("invalid boss-gallery configuration: {error}")),
    }
}

impl BossData {
    pub fn boss_type(&self, activity_id: i32, stage: i32) -> Option<i32> {
        self.stages.iter().find(|r| r.activity_id == activity_id && r.stage == stage)
            .map(|r| r.boss_type)
    }
    pub fn gallery(&self, boss_type: i32) -> Option<&Gallery> {
        self.gallery.iter().find(|r| r.boss_type == boss_type)
    }
}
