use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const CONFIG_TEMPLATE: &str = include_str!("../Config.toml");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub server: ServerSettings,
    #[serde(default)]
    pub muip: MuipConfig,
    #[serde(default)]
    pub muip_gm: MuipGmConfig,
    pub paths: PathConfig,
    pub database: DatabaseConfig,
    pub summon: SummonConfig,
    #[serde(default = "default_tutorial_cards_config")]
    pub tutorial_cards: TutorialCardsConfig,
    #[serde(default = "default_weekwalk_v2_config")]
    pub weekwalk_v2: WeekwalkV2Config,
    #[serde(default = "default_rouge2_config")]
    pub rouge2: Rouge2Config,
    pub activity: ActivityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummonConfig {
    pub permanent_pool_ids: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TutorialCardsConfig {
    pub cards: Vec<TutorialCardConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TutorialCardConfig {
    pub episode_id: i32,
    pub opening_cards: String,
    pub refill_cards: String,
}

fn default_tutorial_cards_config() -> TutorialCardsConfig {
    #[derive(Deserialize)]
    struct Template {
        tutorial_cards: TutorialCardsConfig,
    }

    toml::from_str::<Template>(CONFIG_TEMPLATE)
        .expect("embedded tutorial cards must be valid")
        .tutorial_cards
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekwalkV2Config {
    pub schedule: Vec<WeekwalkV2Schedule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekwalkV2Schedule {
    pub time_id: i32,
    pub start_time: i64,
    pub end_time: i64,
    pub pop_rule: bool,
    pub layers: Vec<WeekwalkV2Layer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekwalkV2Layer {
    pub layer_id: i32,
    pub front_element_id: i32,
    pub rear_element_id: i32,
}

fn default_weekwalk_v2_config() -> WeekwalkV2Config {
    #[derive(Deserialize)]
    struct Template {
        weekwalk_v2: WeekwalkV2Config,
    }

    toml::from_str::<Template>(CONFIG_TEMPLATE)
        .expect("embedded Weekwalk v2 schedule must be valid")
        .weekwalk_v2
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rouge2Config {
    #[serde(default = "default_rouge2_store_currency_id")]
    pub store_currency_id: i32,
    pub initial_attributes: Vec<Rouge2InitialAttribute>,
    pub initial_update_order: Vec<i32>,
}

fn default_rouge2_store_currency_id() -> i32 {
    #[derive(Deserialize)]
    struct Template {
        rouge2: Store,
    }
    #[derive(Deserialize)]
    struct Store {
        store_currency_id: i32,
    }

    toml::from_str::<Template>(CONFIG_TEMPLATE)
        .expect("embedded Rouge2 store currency must be valid")
        .rouge2
        .store_currency_id
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rouge2InitialAttribute {
    pub id: i32,
    pub value: i32,
}

fn default_rouge2_config() -> Rouge2Config {
    #[derive(Deserialize)]
    struct Template {
        rouge2: Rouge2Config,
    }

    toml::from_str::<Template>(CONFIG_TEMPLATE)
        .expect("embedded Rouge2 initialization must be valid")
        .rouge2
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityConfig {
    pub schedule: Vec<ActivitySchedule>,
    #[serde(default = "default_hero_story_schedule")]
    pub hero_story_schedule: Vec<HeroStorySchedule>,
}

fn default_hero_story_schedule() -> Vec<HeroStorySchedule> {
    #[derive(Deserialize)]
    struct Template {
        activity: ActivityConfig,
    }

    toml::from_str::<Template>(CONFIG_TEMPLATE)
        .expect("embedded hero story schedule must be valid")
        .activity
        .hero_story_schedule
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivitySchedule {
    pub id: i32,
    pub start_time: u64,
    pub end_time: u64,
    pub is_unlock: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeroStorySchedule {
    pub story_id: i32,
    pub start_time: i32,
    pub end_time: i32,
    pub start_time_resident: i32,
    pub end_time_resident: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
    pub host: String,
    pub dns: String,
    pub http_port: u16,
    pub game_port: u16,
    #[serde(default)]
    pub skip_tutorial: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MuipConfig {
    pub host: String,
    pub port: u16,
    pub token: String,
    pub gm_host: String,
    pub gm_port: u16,
}

impl Default for MuipConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 21100,
            token: "1999".to_string(),
            gm_host: "127.0.0.1".to_string(),
            gm_port: 21101,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MuipGmConfig {
    pub host: String,
    pub port: u16,
    pub enabled: bool,
}

impl Default for MuipGmConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 21101,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathConfig {
    pub excel_data: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub path: PathBuf,
}

impl ServerConfig {
    pub fn template() -> Self {
        toml::from_str(CONFIG_TEMPLATE).expect("embedded config template must be valid")
    }

    pub fn load_or_create(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, CONFIG_TEMPLATE)?;
        }

        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn resolve_paths(&mut self, config_dir: &Path) {
        if self.database.path.is_relative() {
            self.database.path = config_dir.join(&self.database.path);
        }
        if self.paths.excel_data.is_relative() {
            self.paths.excel_data = config_dir.join(&self.paths.excel_data);
        }
    }

    pub fn validate_paths(&self) -> anyhow::Result<()> {
        if !self.paths.excel_data.exists() {
            anyhow::bail!(
                "excel data directory not found: {}",
                self.paths.excel_data.display()
            );
        }

        if let Some(parent) = self.database.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let permanent_pool_ids = self
            .summon
            .permanent_pool_ids
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        if permanent_pool_ids.len() != self.summon.permanent_pool_ids.len()
            || permanent_pool_ids.iter().any(|pool_id| *pool_id <= 0)
        {
            anyhow::bail!("permanent summon pool IDs must be positive and unique");
        }

        let tutorial_episode_ids = self
            .tutorial_cards
            .cards
            .iter()
            .map(|cards| cards.episode_id)
            .collect::<std::collections::HashSet<_>>();
        if tutorial_episode_ids.len() != self.tutorial_cards.cards.len()
            || self
                .tutorial_cards
                .cards
                .iter()
                .any(|cards| cards.episode_id <= 0 || cards.opening_cards.is_empty())
        {
            anyhow::bail!("tutorial card episode IDs must be positive and unique with an opening");
        }

        if self
            .weekwalk_v2
            .schedule
            .windows(2)
            .any(|rows| rows[0].time_id >= rows[1].time_id)
        {
            anyhow::bail!("Weekwalk v2 schedule IDs must be sorted and unique");
        }

        if self.rouge2.store_currency_id <= 0 {
            anyhow::bail!("Rouge2 store currency ID must be positive");
        }
        let rouge2_attribute_ids = self
            .rouge2
            .initial_attributes
            .iter()
            .map(|attribute| attribute.id)
            .collect::<std::collections::HashSet<_>>();
        if rouge2_attribute_ids.len() != self.rouge2.initial_attributes.len()
            || self
                .rouge2
                .initial_attributes
                .iter()
                .any(|attribute| attribute.id <= 0 || attribute.value < 0)
        {
            anyhow::bail!(
                "Rouge2 initial attributes must have unique positive IDs and nonnegative values"
            );
        }
        let rouge2_update_ids = self
            .rouge2
            .initial_update_order
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        if rouge2_update_ids.len() != self.rouge2.initial_update_order.len()
            || !rouge2_update_ids.is_subset(&rouge2_attribute_ids)
        {
            anyhow::bail!("Rouge2 initial update IDs must be unique configured attributes");
        }
        for row in &self.weekwalk_v2.schedule {
            if row.time_id <= 0 || row.start_time >= row.end_time || row.layers.is_empty() {
                anyhow::bail!("invalid Weekwalk v2 schedule row {}", row.time_id);
            }
            let layer_ids = row
                .layers
                .iter()
                .map(|layer| layer.layer_id)
                .collect::<std::collections::HashSet<_>>();
            if layer_ids.len() != row.layers.len()
                || row.layers.iter().any(|layer| {
                    layer.layer_id <= 0
                        || layer.front_element_id <= 0
                        || layer.rear_element_id <= 0
                        || layer.front_element_id == layer.rear_element_id
                })
            {
                anyhow::bail!("invalid Weekwalk v2 layer mapping {}", row.time_id);
            }
        }

        if self
            .activity
            .schedule
            .windows(2)
            .any(|rows| rows[0].id >= rows[1].id)
        {
            anyhow::bail!("activity schedule IDs must be sorted and unique");
        }
        if let Some(row) = self
            .activity
            .schedule
            .iter()
            .find(|row| row.id <= 0 || row.start_time >= row.end_time)
        {
            anyhow::bail!("invalid activity schedule row {}", row.id);
        }
        if self
            .activity
            .hero_story_schedule
            .windows(2)
            .any(|rows| rows[0].story_id >= rows[1].story_id)
        {
            anyhow::bail!("hero story schedule IDs must be sorted and unique");
        }
        if let Some(row) = self.activity.hero_story_schedule.iter().find(|row| {
            row.story_id <= 0
                || row.start_time >= row.end_time
                || row.start_time_resident >= row.end_time_resident
        }) {
            anyhow::bail!("invalid hero story schedule row {}", row.story_id);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn load_or_create_writes_missing_config() {
        let name = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("enigma-config-{name}"));
        let path = dir.join("config.toml");

        let cfg = ServerConfig::load_or_create(&path).unwrap();

        assert!(path.exists());
        assert_eq!(cfg.server.http_port, 21000);
        assert_eq!(cfg.server.game_port, 23301);
        assert!(!cfg.server.skip_tutorial);
        assert_eq!(cfg.muip.port, 21100);
        assert_eq!(cfg.muip_gm.port, 21101);
        assert_eq!(cfg.summon.permanent_pool_ids, [2]);
        assert_eq!(cfg.tutorial_cards.cards.len(), 4);
        assert_eq!(cfg.tutorial_cards.cards[1].episode_id, 10002);
        assert_eq!(
            cfg.tutorial_cards.cards[1].opening_cards,
            "100102#2|100102#2|100102#2|100101#1|100101#1|100101#2|100102#2"
        );
        assert_eq!(cfg.weekwalk_v2.schedule[0].time_id, 21);
        assert_eq!(cfg.weekwalk_v2.schedule[0].layers[0].layer_id, 1069);
        assert_eq!(cfg.rouge2.initial_attributes.len(), 61);
        assert_eq!(cfg.rouge2.initial_update_order.len(), 16);
        assert!(cfg.activity.schedule.iter().any(|row| row.id == 13930));
        assert_eq!(cfg.activity.hero_story_schedule.len(), 33);

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_weekwalk_v2_schedule_uses_embedded_default() {
        let mut template: toml::Value = toml::from_str(CONFIG_TEMPLATE).unwrap();
        template.as_table_mut().unwrap().remove("weekwalk_v2");

        let config: ServerConfig = template.try_into().unwrap();
        assert_eq!(config.weekwalk_v2.schedule[0].time_id, 21);
    }

    #[test]
    fn missing_tutorial_cards_use_embedded_defaults() {
        let mut template: toml::Value = toml::from_str(CONFIG_TEMPLATE).unwrap();
        template.as_table_mut().unwrap().remove("tutorial_cards");

        let config: ServerConfig = template.try_into().unwrap();
        assert_eq!(config.tutorial_cards.cards.len(), 4);
        assert_eq!(config.tutorial_cards.cards[0].episode_id, 10001);
    }

    #[test]
    fn missing_rouge2_initialization_uses_embedded_default() {
        let mut template: toml::Value = toml::from_str(CONFIG_TEMPLATE).unwrap();
        template.as_table_mut().unwrap().remove("rouge2");

        let config: ServerConfig = template.try_into().unwrap();
        assert_eq!(config.rouge2.initial_attributes.len(), 61);
        assert_eq!(config.rouge2.initial_update_order[0], 320);
    }

    #[test]
    fn missing_rouge2_store_currency_uses_embedded_default() {
        let mut template: toml::Value = toml::from_str(CONFIG_TEMPLATE).unwrap();
        template["rouge2"]
            .as_table_mut()
            .unwrap()
            .remove("store_currency_id");

        let config: ServerConfig = template.try_into().unwrap();
        assert_eq!(config.rouge2.store_currency_id, 3201);
    }

    #[test]
    fn invalid_rouge2_initialization_is_rejected() {
        let validate = |mut config: ServerConfig| {
            config.paths.excel_data = std::env::temp_dir();
            config.database.path = std::env::temp_dir().join("enigma-config-test.db");
            config.validate_paths()
        };

        let mut duplicate = ServerConfig::template();
        duplicate
            .rouge2
            .initial_attributes
            .push(duplicate.rouge2.initial_attributes[0].clone());
        assert!(validate(duplicate).is_err());

        let mut unknown_update = ServerConfig::template();
        unknown_update.rouge2.initial_update_order.push(999);
        assert!(validate(unknown_update).is_err());

        let mut negative_value = ServerConfig::template();
        negative_value.rouge2.initial_attributes[0].value = -1;
        assert!(validate(negative_value).is_err());
    }

    #[test]
    fn invalid_weekwalk_v2_schedule_is_rejected() {
        let mut duplicate = ServerConfig::template();
        duplicate.paths.excel_data = std::env::temp_dir();
        duplicate.database.path = std::env::temp_dir().join("enigma-config-test.db");
        duplicate
            .weekwalk_v2
            .schedule
            .push(duplicate.weekwalk_v2.schedule[0].clone());
        assert!(duplicate.validate_paths().is_err());

        let mut invalid_layer = ServerConfig::template();
        invalid_layer.paths.excel_data = std::env::temp_dir();
        invalid_layer.database.path = std::env::temp_dir().join("enigma-config-test.db");
        invalid_layer.weekwalk_v2.schedule[0].layers[0].rear_element_id =
            invalid_layer.weekwalk_v2.schedule[0].layers[0].front_element_id;
        assert!(invalid_layer.validate_paths().is_err());
    }

    #[test]
    fn missing_hero_story_schedule_uses_embedded_default() {
        let activity: ActivityConfig = toml::from_str("schedule = []").unwrap();

        assert_eq!(activity.hero_story_schedule.len(), 33);
    }

    #[test]
    fn configured_hero_story_schedule_replaces_default() {
        let activity: ActivityConfig = toml::from_str(
            r#"
            schedule = []
            hero_story_schedule = [
              { story_id = 99, start_time = 1, end_time = 2, start_time_resident = 3, end_time_resident = 4 }
            ]
            "#,
        )
        .unwrap();

        assert_eq!(activity.hero_story_schedule.len(), 1);
        assert_eq!(activity.hero_story_schedule[0].story_id, 99);
    }
}
