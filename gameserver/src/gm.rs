use muipserver::{GmRequest, GmResponse};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};
use tracing::{info, warn};

use crate::net::app::AppState;

mod catalog;
mod command;
mod editor;
#[cfg(test)]
mod tests;

use catalog::{dungeon_catalog, hero_upgrade_catalog, materials};
use command::run_command;

pub async fn run_gm_listener(addr: String, state: &'static AppState) -> std::io::Result<()> {
    let listener = TcpListener::bind(&addr).await?;
    info!("MUIP GM bridge listening on {}", listener.local_addr()?);

    loop {
        let (stream, peer) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(err) = handle_connection(stream, state).await {
                warn!("MUIP GM connection {peer} failed: {err}");
            }
        });
    }
}

async fn handle_connection(stream: TcpStream, state: &'static AppState) -> std::io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    reader.read_line(&mut line).await?;

    let trimmed = line.trim();
    if trimmed.is_empty() {
        return write_response(&mut writer, GmResponse::err(400, "empty GM request")).await;
    }
    if is_http_request(trimmed) {
        return write_http_redirect(&mut writer).await;
    }

    let request = match serde_json::from_str::<GmRequest>(trimmed) {
        Ok(request) => request,
        Err(err) => {
            return write_response(
                &mut writer,
                GmResponse::err(400, format!("invalid GM request: {err}")),
            )
            .await;
        }
    };

    let response = match request {
        GmRequest::Status => status(state).await,
        GmRequest::ListPlayers => list_players(state),
        GmRequest::Dungeons => dungeon_catalog(),
        GmRequest::Events => match editor::event_catalog(state) {
            Ok(catalog) => GmResponse::ok_data("events", catalog),
            Err(err) => GmResponse::err(400, err.to_string()),
        },
        GmRequest::Heroes { player_uid } => hero_upgrade_catalog(state, player_uid).await,
        GmRequest::Equipment { player_uid } => {
            match editor::equipment_catalog(state, player_uid).await {
                Ok(catalog) => GmResponse::ok_data("equipment", catalog),
                Err(err) => GmResponse::err(400, err.to_string()),
            }
        }
        GmRequest::Materials { query } => materials(state, query).await,
        GmRequest::UpdateEvent {
            id,
            enabled,
            start_time,
            end_time,
        } => match editor::update_event(state, id, enabled, start_time, end_time) {
            Ok(event) => GmResponse::ok_data(
                "updated event schedule; restart the servers to apply it",
                event,
            ),
            Err(err) => GmResponse::err(400, err.to_string()),
        },
        GmRequest::Execute {
            player_uid,
            command,
        } => execute(state, player_uid, command).await,
    };

    write_response(&mut writer, response).await
}

async fn write_response(
    writer: &mut (impl AsyncWriteExt + Unpin),
    response: GmResponse,
) -> std::io::Result<()> {
    let mut payload = serde_json::to_vec(&response).map_err(std::io::Error::other)?;
    payload.push(b'\n');
    writer.write_all(&payload).await?;
    writer.flush().await
}

fn is_http_request(line: &str) -> bool {
    line.starts_with("GET ") || line.starts_with("HEAD ")
}

async fn write_http_redirect(writer: &mut (impl AsyncWriteExt + Unpin)) -> std::io::Result<()> {
    let location = format!("http://127.0.0.1:{}/", common::muip_port());
    let body = format!(
        "<!doctype html><meta http-equiv=\"refresh\" content=\"0;url={0}\"><a href=\"{0}\">Open MUIP panel</a>",
        location
    );
    let response = format!(
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    writer.write_all(response.as_bytes()).await?;
    writer.flush().await
}

async fn status(state: &AppState) -> GmResponse {
    let online_players = state.online_player_ids();
    let player_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
        .fetch_one(state.db)
        .await
        .unwrap_or_default() as usize;

    let mut response = GmResponse::ok("online");
    response.online = online_players.len();
    response.players = online_players
        .into_iter()
        .map(|id| id.to_string())
        .collect();
    response.data = Some(serde_json::json!({
        "online": response.online,
        "playerCount": player_count,
        "maxPlayers": 99999999,
        "players": response.players,
        "status": "online"
    }));
    response
}

fn list_players(state: &AppState) -> GmResponse {
    let players = state
        .online_player_ids()
        .into_iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>();

    let mut response = GmResponse::ok(format!("{} player(s) online", players.len()));
    response.online = players.len();
    response.players = players;
    response
}

async fn execute(state: &'static AppState, player_uid: String, command: String) -> GmResponse {
    let Ok(player_id) = player_uid.parse::<i64>() else {
        return GmResponse::err(400, format!("invalid player_uid `{player_uid}`"));
    };

    let exists = match sqlx::query_scalar::<_, i64>("SELECT 1 FROM users WHERE id = ?")
        .bind(player_id)
        .fetch_optional(state.db)
        .await
    {
        Ok(exists) => exists,
        Err(err) => return GmResponse::err(500, format!("database error: {err}")),
    };
    if exists.is_none() {
        return GmResponse::err(404, format!("player `{player_uid}` was not found"));
    }

    match run_command(state, player_id, &command).await {
        Ok(response) => response,
        Err(err) => GmResponse::err(400, err.to_string()),
    }
}
