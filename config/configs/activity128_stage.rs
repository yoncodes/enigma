// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity128Stage {
    #[serde(rename = "activityId")]
    pub activity_id: i32,
    #[serde(rename = "bossRushLevelDetailFullBgSimage")]
    pub boss_rush_level_detail_full_bg_simage: String,
    #[serde(rename = "bossRushMainBg")]
    pub boss_rush_main_bg: String,
    #[serde(rename = "bossRushMainItemBossSprite")]
    pub boss_rush_main_item_boss_sprite: String,
    #[serde(rename = "heartVariantId")]
    pub heart_variant_id: i32,
    #[serde(rename = "isActivity")]
    pub is_activity: bool,
    #[serde(rename = "layer4MaxPoints")]
    pub layer4_max_points: i32,
    #[serde(rename = "maxPoints")]
    pub max_points: i32,
    pub name: String,
    pub name_en: String,
    #[serde(rename = "openDay")]
    pub open_day: i32,
    #[serde(rename = "resultViewFullBgSImage")]
    pub result_view_full_bg_simage: String,
    #[serde(rename = "resultViewNameSImage")]
    pub result_view_name_simage: String,
    #[serde(rename = "skinIds")]
    pub skin_ids: String,
    #[serde(rename = "skinOffsetXYs")]
    pub skin_offset_xys: String,
    #[serde(rename = "skinScales")]
    pub skin_scales: String,
    pub stage: i32,
    #[serde(rename = "type")]
    pub r#type: i32,
    pub version: i32,
}
pub struct Activity128StageTable {
    records: Vec<Activity128Stage>,
}

impl Activity128StageTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity128Stage> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[Activity128Stage] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity128Stage> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}