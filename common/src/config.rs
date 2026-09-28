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
    pub activity: ActivityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummonConfig {
    pub permanent_pool_ids: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityConfig {
    pub schedule: Vec<ActivitySchedule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivitySchedule {
    pub id: i32,
    pub start_time: u64,
    pub end_time: u64,
    pub is_unlock: bool,
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
        assert!(cfg.activity.schedule.iter().any(|row| row.id == 13801));

        std::fs::remove_dir_all(dir).unwrap();
    }
}
