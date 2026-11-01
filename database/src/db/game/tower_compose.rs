use anyhow::{Result, ensure};
use sonettobuf::TowerComposePlaneMods;
use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};
use std::collections::HashSet;

#[derive(Debug, Clone, FromRow)]
pub struct TowerComposeThemeState {
    pub theme_id: i32,
    pub research_progress: i32,
    pub pass_max_layer_id: i32,
    pub high_score: i32,
    pub curr_score: i32,
    pub boss_level: i32,
    pub boss_lock: bool,
    pub saved_record: bool,
    pub params: String,
}

pub async fn get_theme_states(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<Vec<TowerComposeThemeState>> {
    Ok(sqlx::query_as::<_, TowerComposeThemeState>(
        r#"
        SELECT theme_id, research_progress, pass_max_layer_id, high_score, curr_score,
               boss_level, boss_lock, saved_record, params
        FROM user_tower_compose_theme_state
        WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_theme_state(
    pool: &SqlitePool,
    user_id: i64,
    theme_id: i32,
) -> Result<Option<TowerComposeThemeState>> {
    Ok(sqlx::query_as::<_, TowerComposeThemeState>(
        r#"
        SELECT theme_id, research_progress, pass_max_layer_id, high_score, curr_score,
               boss_level, boss_lock, saved_record, params
        FROM user_tower_compose_theme_state
        WHERE user_id = ? AND theme_id = ?
        "#,
    )
    .bind(user_id)
    .bind(theme_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn get_theme_state_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    theme_id: i32,
) -> Result<Option<TowerComposeThemeState>> {
    Ok(sqlx::query_as::<_, TowerComposeThemeState>(
        r#"
        SELECT theme_id, research_progress, pass_max_layer_id, high_score, curr_score,
               boss_level, boss_lock, saved_record, params
        FROM user_tower_compose_theme_state
        WHERE user_id = ? AND theme_id = ?
        "#,
    )
    .bind(user_id)
    .bind(theme_id)
    .fetch_optional(&mut **tx)
    .await?)
}

pub async fn complete_layer_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    theme_id: i32,
    layer_id: i32,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO user_tower_compose_theme_state
            (user_id, theme_id, pass_max_layer_id, updated_at)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(user_id, theme_id) DO UPDATE SET
            pass_max_layer_id = MAX(pass_max_layer_id, excluded.pass_max_layer_id),
            updated_at = excluded.updated_at
        "#,
    )
    .bind(user_id)
    .bind(theme_id)
    .bind(layer_id)
    .bind(common::time::ServerTime::now_ms())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn save_plane_mods(
    pool: &SqlitePool,
    user_id: i64,
    theme_id: i32,
    plane_mods: &[TowerComposePlaneMods],
) -> Result<()> {
    let now = common::time::ServerTime::now_ms();
    let mut tx = pool.begin().await?;
    let mut seen = HashSet::new();
    for plane in plane_mods {
        let plane_id = plane.plane_id.unwrap_or_default();
        ensure!((1..=2).contains(&plane_id) && seen.insert(plane_id), "invalid or duplicate compose plane");
        sqlx::query(
            r#"
            INSERT INTO user_tower_compose_plane_mods
                (user_id, theme_id, plane_id, mods_json, updated_at)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(user_id, theme_id, plane_id) DO UPDATE SET
                mods_json = excluded.mods_json,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(user_id)
        .bind(theme_id)
        .bind(plane_id)
        .bind(serde_json::to_string(&plane.mods)?)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}
