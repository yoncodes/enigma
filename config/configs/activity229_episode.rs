// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity229Episode {
    #[serde(rename = "activityId")]
    pub activity_id: i32,
    #[serde(rename = "careerPrefer")]
    pub career_prefer: String,
    #[serde(rename = "episodeId")]
    pub episode_id: i32,
    #[serde(rename = "optionalSkills")]
    pub optional_skills: String,
    pub stage: i32,
    #[serde(rename = "teamRecommend")]
    pub team_recommend: String,
}
pub struct Activity229EpisodeTable {
    records: Vec<Activity229Episode>,
}

impl Activity229EpisodeTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Activity229Episode> = crate::load_rows(path)?;

        Ok(Self {
            records,
        })
    }

    #[inline]
    pub fn all(&self) -> &[Activity229Episode] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Activity229Episode> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}