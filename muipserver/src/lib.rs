mod routes;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tracing::info;

#[derive(Debug, Clone)]
pub struct MuipOptions {
    pub host: String,
    pub port: u16,
    pub token: String,
    pub gm_addr: String,
}

impl MuipOptions {
    pub fn from_config() -> Self {
        Self {
            host: common::muip_host().to_string(),
            port: common::muip_port(),
            token: common::muip_token().to_string(),
            gm_addr: common::muip_gm_addr(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GmRequest {
    Status,
    ListPlayers,
    Dungeons,
    Heroes { player_uid: i64 },
    Materials { query: MaterialQuery },
    Execute { player_uid: String, command: String },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GmResponse {
    pub retcode: i32,
    pub message: String,
    #[serde(default)]
    pub online: usize,
    #[serde(default)]
    pub players: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl GmResponse {
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            retcode: 0,
            message: message.into(),
            ..Default::default()
        }
    }

    pub fn ok_data(message: impl Into<String>, data: impl Serialize) -> Self {
        Self {
            retcode: 0,
            message: message.into(),
            data: Some(serde_json::to_value(data).unwrap_or(serde_json::Value::Null)),
            ..Default::default()
        }
    }

    pub fn err(retcode: i32, message: impl Into<String>) -> Self {
        Self {
            retcode,
            message: message.into(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MaterialQuery {
    pub r#type: Option<i32>,
    pub q: Option<String>,
    pub limit: Option<usize>,
    pub player_uid: Option<i64>,
    pub unowned_only: Option<bool>,
}

pub async fn run(options: MuipOptions) -> anyhow::Result<()> {
    let addr = format!("{}:{}", options.host, options.port);
    let listener = TcpListener::bind(&addr)
        .await
        .with_context(|| format!("failed to bind MUIP server on {addr}"))?;

    info!("MUIP HTTP server listening on {}", listener.local_addr()?);
    axum::serve(listener, routes::router(options.token, options.gm_addr)).await?;
    Ok(())
}
