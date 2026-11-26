use crate::models::game::red_dots::{RedDotGroup, RedDotRecord};
use anyhow::Result;
use sqlx::SqlitePool;
use std::collections::HashMap;

pub async fn get_red_dots(pool: &SqlitePool, player_id: i64) -> Result<Vec<RedDotRecord>> {
    let dots = sqlx::query_as::<_, RedDotRecord>(
        "SELECT * FROM red_dots WHERE player_id = ? ORDER BY define_id, info_id",
    )
    .bind(player_id)
    .fetch_all(pool)
    .await?;
    Ok(dots)
}

pub async fn get_red_dots_by_defines(
    pool: &SqlitePool,
    player_id: i64,
    define_ids: Vec<i32>,
) -> Result<Vec<RedDotRecord>> {
    if define_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = define_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let query = format!(
        "SELECT * FROM red_dots WHERE player_id = ? AND define_id IN ({}) ORDER BY define_id, info_id",
        placeholders
    );

    let mut q = sqlx::query_as::<_, RedDotRecord>(&query).bind(player_id);
    for id in define_ids {
        q = q.bind(id);
    }

    Ok(q.fetch_all(pool).await?)
}

pub fn group_red_dots(records: Vec<RedDotRecord>) -> Vec<RedDotGroup> {
    let mut grouped: HashMap<i32, Vec<RedDotRecord>> = HashMap::new();

    for record in records {
        grouped.entry(record.define_id).or_default().push(record);
    }

    grouped
        .into_iter()
        .map(|(define_id, dots)| {
            let replace_all = dots.first().map(|d| d.replace_all).unwrap_or(true);
            RedDotGroup {
                define_id,
                dots,
                replace_all,
            }
        })
        .collect()
}

pub struct RedDotValue {
    pub value: i32,
    pub time: i64,
    pub ext: String,
    pub replace_all: bool,
}

pub async fn upsert_red_dot(
    pool: &SqlitePool,
    player_id: i64,
    define_id: i32,
    info_id: i32,
    dot: RedDotValue,
) -> Result<()> {
    let now = common::time::ServerTime::now_ms();

    sqlx::query(
        "INSERT INTO red_dots (
            player_id, define_id, info_id, value, time, ext, replace_all, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(player_id, define_id, info_id)
        DO UPDATE SET
            value = excluded.value,
            time = excluded.time,
            ext = excluded.ext,
            replace_all = excluded.replace_all,
            updated_at = excluded.updated_at",
    )
    .bind(player_id)
    .bind(define_id)
    .bind(info_id)
    .bind(dot.value)
    .bind(dot.time)
    .bind(dot.ext)
    .bind(dot.replace_all)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn clear_red_dots_by_define(
    pool: &SqlitePool,
    player_id: i64,
    define_id: i32,
) -> Result<()> {
    sqlx::query("DELETE FROM red_dots WHERE player_id = ? AND define_id = ?")
        .bind(player_id)
        .bind(define_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn hide_red_dot_infos(
    pool: &SqlitePool,
    player_id: i64,
    define_id: i32,
    mut info_ids: Vec<i32>,
) -> Result<Vec<i32>> {
    if info_ids.is_empty() {
        info_ids.push(0);
    }

    let now = common::time::ServerTime::now_ms();
    for info_id in info_ids.clone() {
        sqlx::query(
            r#"
            INSERT INTO red_dots (
                player_id, define_id, info_id, value, time, ext,
                replace_all, created_at, updated_at
            ) VALUES (?, ?, ?, 0, 0, '', 0, ?, ?)
            ON CONFLICT(player_id, define_id, info_id) DO UPDATE SET
                value = 0,
                time = 0,
                ext = '',
                replace_all = 0,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(player_id)
        .bind(define_id)
        .bind(info_id)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;
    }

    Ok(info_ids)
}

pub async fn hide_visible_red_dot_info(
    pool: &SqlitePool,
    player_id: i64,
    define_id: i32,
    info_id: i32,
) -> Result<Option<RedDotRecord>> {
    let now = common::time::ServerTime::now_ms();
    Ok(sqlx::query_as::<_, RedDotRecord>(
        "UPDATE red_dots
         SET value = 0, updated_at = ?
         WHERE player_id = ? AND define_id = ? AND info_id = ? AND value != 0
         RETURNING *",
    )
    .bind(now)
    .bind(player_id)
    .bind(define_id)
    .bind(info_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn clear_all_red_dots(pool: &SqlitePool, player_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM red_dots WHERE player_id = ?")
        .bind(player_id)
        .execute(pool)
        .await?;
    Ok(())
}
