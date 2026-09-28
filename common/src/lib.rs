use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

pub mod config;
pub mod time;
pub mod types;

static CONFIG: OnceLock<config::ServerConfig> = OnceLock::new();

pub fn init_config(config: config::ServerConfig) {
    CONFIG.set(config).expect("config already initialized");
}

pub fn config() -> &'static config::ServerConfig {
    CONFIG.get().expect("config not initialized")
}

pub fn host() -> &'static str {
    &config().server.host
}

pub fn http_port() -> u16 {
    config().server.http_port
}

pub fn game_port() -> u16 {
    config().server.game_port
}

pub fn skip_tutorial() -> bool {
    config().server.skip_tutorial
}

pub fn muip_host() -> &'static str {
    &config().muip.host
}

pub fn muip_port() -> u16 {
    config().muip.port
}

pub fn muip_token() -> &'static str {
    &config().muip.token
}

pub fn muip_gm_addr() -> String {
    format!("{}:{}", config().muip.gm_host, config().muip.gm_port)
}

pub fn muip_gm_listen_addr() -> String {
    format!("{}:{}", config().muip_gm.host, config().muip_gm.port)
}

pub fn muip_gm_enabled() -> bool {
    config().muip_gm.enabled
}

pub fn excel_data_directory() -> &'static PathBuf {
    &config().paths.excel_data
}

pub fn activity_schedule() -> &'static [config::ActivitySchedule] {
    &config().activity.schedule
}

pub fn init_tracing() {
    #[cfg(target_os = "windows")]
    let _ = ansi_term::enable_ansi_support();

    let _ = tracing_subscriber::fmt().try_init();
}

pub fn load_config() -> anyhow::Result<config::ServerConfig> {
    let config_path = config_path();
    let mut cfg = config::ServerConfig::load_or_create(&config_path)?;
    let config_dir = config_path.parent().unwrap_or_else(|| Path::new("."));

    cfg.resolve_paths(config_dir);
    cfg.validate_paths()?;
    Ok(cfg)
}

fn config_path() -> PathBuf {
    if let Ok(path) = std::env::var("ENIGMA_CONFIG") {
        return PathBuf::from(path);
    }

    let cwd_path = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("config.toml");
    if cwd_path.exists() {
        return cwd_path;
    }

    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.join("config.toml")))
        .unwrap_or(cwd_path)
}
