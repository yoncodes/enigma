use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, head, post},
};
use serde::Deserialize;
use std::sync::Arc;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};
use tracing::error;

use crate::{GmRequest, GmResponse, MaterialQuery};

static PANEL_HTML: &str = include_str!("../res/index.html");

#[derive(Clone)]
struct AppState {
    token: Arc<str>,
    gm_addr: Arc<str>,
}

#[derive(Debug, Deserialize)]
struct GmQuery {
    token: String,
    player_uid: Option<String>,
    command: String,
}

#[derive(Debug, Deserialize)]
struct GmBody {
    token: String,
    player_uid: Option<String>,
    command: String,
}

#[derive(Debug, Deserialize)]
struct HeroQuery {
    token: String,
    player_uid: i64,
}

pub fn router(token: String, gm_addr: String) -> Router {
    let state = AppState {
        token: Arc::from(token),
        gm_addr: Arc::from(gm_addr),
    };

    Router::new()
        .route("/", head(root_head).get(panel_handler))
        .route("/status", get(status_handler))
        .route("/status/server", get(status_handler))
        .route("/api/status", get(status_handler))
        .route("/api/players", get(players_handler))
        .route("/api/sessions", get(players_handler))
        .route("/api/dungeons", get(dungeons_handler))
        .route("/api/heroes", get(heroes_handler))
        .route("/api/materials", get(materials_handler))
        .route("/muip/gm", get(gm_query_handler))
        .route("/api/run_gm_cmd", post(gm_body_handler))
        .with_state(state)
}

async fn root_head() -> StatusCode {
    StatusCode::OK
}

async fn panel_handler(State(state): State<AppState>) -> Html<String> {
    Html(
        PANEL_HTML
            .replace("__MUIP_TOKEN__", &state.token)
            .replace("__GM_ADDR__", &state.gm_addr),
    )
}

async fn status_handler(State(state): State<AppState>) -> Response {
    forward_response(&state.gm_addr, GmRequest::Status).await
}

async fn players_handler(State(state): State<AppState>) -> Response {
    forward_response(&state.gm_addr, GmRequest::ListPlayers).await
}

async fn dungeons_handler(State(state): State<AppState>) -> Response {
    match send_gm(&state.gm_addr, GmRequest::Dungeons).await {
        Ok(response) => (
            StatusCode::OK,
            Json(response.data.unwrap_or_else(|| serde_json::json!({}))),
        )
            .into_response(),
        Err(err) => {
            error!("GM forwarding failed: {err}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "chapters": [],
                    "episodes": [],
                    "error": format!("GM bridge unavailable: {err}")
                })),
            )
                .into_response()
        }
    }
}

async fn heroes_handler(State(state): State<AppState>, Query(query): Query<HeroQuery>) -> Response {
    if query.token != *state.token {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "heroes": [], "error": "invalid MUIP token" })),
        )
            .into_response();
    }
    match send_gm(
        &state.gm_addr,
        GmRequest::Heroes {
            player_uid: query.player_uid,
        },
    )
    .await
    {
        Ok(response) if response.retcode == 0 => (
            StatusCode::OK,
            Json(response.data.unwrap_or_else(|| serde_json::json!({}))),
        )
            .into_response(),
        Ok(response) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "heroes": [], "error": response.message })),
        )
            .into_response(),
        Err(err) => {
            error!("GM forwarding failed: {err}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "heroes": [],
                    "error": format!("GM bridge unavailable: {err}")
                })),
            )
                .into_response()
        }
    }
}

async fn materials_handler(
    State(state): State<AppState>,
    Query(query): Query<MaterialQuery>,
) -> Response {
    match send_gm(&state.gm_addr, GmRequest::Materials { query }).await {
        Ok(response) => (
            StatusCode::OK,
            Json(response.data.unwrap_or_else(|| serde_json::json!({}))),
        )
            .into_response(),
        Err(err) => {
            error!("GM forwarding failed: {err}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "types": [],
                    "items": [],
                    "error": format!("GM bridge unavailable: {err}")
                })),
            )
                .into_response()
        }
    }
}

async fn gm_query_handler(State(state): State<AppState>, Query(query): Query<GmQuery>) -> Response {
    gm_handler(state, query.token, query.player_uid, query.command).await
}

async fn gm_body_handler(State(state): State<AppState>, Json(body): Json<GmBody>) -> Response {
    gm_handler(state, body.token, body.player_uid, body.command).await
}

async fn gm_handler(
    state: AppState,
    token: String,
    player_uid: Option<String>,
    command: String,
) -> Response {
    if token != *state.token {
        return (
            StatusCode::UNAUTHORIZED,
            Json(GmResponse::err(401, "invalid MUIP token")),
        )
            .into_response();
    }

    let request = match command.trim().to_ascii_lowercase().as_str() {
        "help" | "?" => {
            return (
                StatusCode::OK,
                Json(GmResponse::ok(
                    "commands: help, status, players, bgm unlock all, guide complete all, hero upgrade materials <hero id> <1-180> <resonance 1-15> [destiny rank] [destiny stone id], dungeon unlock <stage|chapter> <id>, material <type> <id> <amount>, give <item|currency|hero|skin|equip|power|insight> <id> <amount>",
                )),
            )
                .into_response();
        }
        "status" => GmRequest::Status,
        "info" | "list" | "players" | "listplayers" | "list_players" => GmRequest::ListPlayers,
        _ => {
            let Some(player_uid) = player_uid else {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(GmResponse::err(
                        400,
                        "player_uid is required for this command",
                    )),
                )
                    .into_response();
            };

            GmRequest::Execute {
                player_uid,
                command,
            }
        }
    };

    forward_response(&state.gm_addr, request).await
}

async fn forward_response(addr: &str, request: GmRequest) -> Response {
    match send_gm(addr, request).await {
        Ok(response) => {
            let status = if response.retcode == 0 {
                StatusCode::OK
            } else {
                StatusCode::from_u16(response.retcode as u16)
                    .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
            };
            (status, Json(response)).into_response()
        }
        Err(err) => {
            error!("GM forwarding failed: {err}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(GmResponse::err(
                    503,
                    format!("GM bridge unavailable: {err}"),
                )),
            )
                .into_response()
        }
    }
}

async fn send_gm(addr: &str, request: GmRequest) -> anyhow::Result<GmResponse> {
    let mut stream = TcpStream::connect(addr).await?;

    let mut payload = serde_json::to_vec(&request)?;
    payload.push(b'\n');
    stream.write_all(&payload).await?;
    stream.flush().await?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await?;

    let trimmed = line.trim();
    if trimmed.is_empty() {
        anyhow::bail!("empty GM response");
    }

    Ok(serde_json::from_str(trimmed)?)
}
