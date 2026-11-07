use crate::models::game::explore::state::MapState;
use anyhow::{Result, ensure};
use sqlx::SqliteConnection;

pub async fn last_map(conn: &mut SqliteConnection, user_id: i64) -> Result<Option<i32>> {
    Ok(
        sqlx::query_scalar("SELECT last_map_id FROM user_explore_info WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(conn)
            .await?,
    )
}

pub async fn load(
    conn: &mut SqliteConnection,
    user_id: i64,
    map_id: i32,
) -> Result<Option<MapState>> {
    let snapshot: Option<String> = sqlx::query_scalar(
        "SELECT snapshot FROM user_explore_map_state WHERE user_id = ? AND map_id = ?",
    )
    .bind(user_id)
    .bind(map_id)
    .fetch_optional(conn)
    .await?;
    let state = snapshot
        .map(|snapshot| serde_json::from_str::<MapState>(&snapshot))
        .transpose()?;
    if let Some(state) = &state {
        ensure!(
            state.map_id == map_id,
            "exploration snapshot belongs to another map"
        );
    }
    Ok(state)
}

pub async fn insert(conn: &mut SqliteConnection, user_id: i64, state: &MapState) -> Result<bool> {
    Ok(sqlx::query(
        "INSERT INTO user_explore_map_state (user_id, map_id, snapshot)
         VALUES (?, ?, ?) ON CONFLICT(user_id, map_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(state.map_id)
    .bind(serde_json::to_string(state)?)
    .execute(conn)
    .await?
    .rows_affected()
        == 1)
}

pub async fn update(conn: &mut SqliteConnection, user_id: i64, state: &MapState) -> Result<bool> {
    Ok(sqlx::query(
        "UPDATE user_explore_map_state SET snapshot = ?
             WHERE user_id = ? AND map_id = ?",
    )
    .bind(serde_json::to_string(state)?)
    .bind(user_id)
    .bind(state.map_id)
    .execute(conn)
    .await?
    .rows_affected()
        == 1)
}

pub async fn select(conn: &mut SqliteConnection, user_id: i64, map_id: i32) -> Result<()> {
    sqlx::query(
        "INSERT INTO user_explore_info (user_id, last_map_id) VALUES (?, ?)
         ON CONFLICT(user_id) DO UPDATE SET last_map_id = excluded.last_map_id",
    )
    .bind(user_id)
    .bind(map_id)
    .execute(conn)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests;
