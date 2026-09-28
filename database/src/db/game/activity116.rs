use anyhow::Result;
use sqlx::{Sqlite, SqlitePool, Transaction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity116State {
    pub elements: Vec<(i32, i32)>,
    pub trap_ids: Vec<i32>,
    pub put_trap: i32,
}

pub async fn get_or_create_state(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
) -> Result<Activity116State> {
    sqlx::query(
        "INSERT OR IGNORE INTO user_activity116_state (user_id, activity_id)
         VALUES (?, ?)",
    )
    .bind(user_id)
    .bind(activity_id)
    .execute(pool)
    .await?;

    let put_trap = sqlx::query_scalar(
        "SELECT put_trap FROM user_activity116_state
         WHERE user_id = ? AND activity_id = ?",
    )
    .bind(user_id)
    .bind(activity_id)
    .fetch_one(pool)
    .await?;
    let elements = sqlx::query_as(
        "SELECT element_id, level FROM user_activity116_elements
         WHERE user_id = ? AND activity_id = ? ORDER BY element_id",
    )
    .bind(user_id)
    .bind(activity_id)
    .fetch_all(pool)
    .await?;
    let trap_ids = sqlx::query_scalar(
        "SELECT trap_id FROM user_activity116_traps
         WHERE user_id = ? AND activity_id = ? ORDER BY trap_id",
    )
    .bind(user_id)
    .bind(activity_id)
    .fetch_all(pool)
    .await?;

    Ok(Activity116State {
        elements,
        trap_ids,
        put_trap,
    })
}

pub async fn element_level(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
    element_id: i32,
) -> Result<i32> {
    Ok(sqlx::query_scalar(
        "SELECT level FROM user_activity116_elements
         WHERE user_id = ? AND activity_id = ? AND element_id = ?",
    )
    .bind(user_id)
    .bind(activity_id)
    .bind(element_id)
    .fetch_optional(pool)
    .await?
    .unwrap_or_default())
}

async fn ensure_state(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    activity_id: i32,
) -> Result<()> {
    sqlx::query(
        "INSERT OR IGNORE INTO user_activity116_state (user_id, activity_id)
         VALUES (?, ?)",
    )
    .bind(user_id)
    .bind(activity_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn upgrade_element(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    activity_id: i32,
    element_id: i32,
    current_level: i32,
) -> Result<bool> {
    ensure_state(tx, user_id, activity_id).await?;
    sqlx::query(
        "INSERT OR IGNORE INTO user_activity116_elements
         (user_id, activity_id, element_id, level) VALUES (?, ?, ?, 0)",
    )
    .bind(user_id)
    .bind(activity_id)
    .bind(element_id)
    .execute(&mut **tx)
    .await?;
    Ok(sqlx::query(
        "UPDATE user_activity116_elements SET level = level + 1
         WHERE user_id = ? AND activity_id = ? AND element_id = ? AND level = ?",
    )
    .bind(user_id)
    .bind(activity_id)
    .bind(element_id)
    .bind(current_level)
    .execute(&mut **tx)
    .await?
    .rows_affected()
        == 1)
}

pub async fn build_trap(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    activity_id: i32,
    trap_id: i32,
) -> Result<bool> {
    ensure_state(tx, user_id, activity_id).await?;
    Ok(sqlx::query(
        "INSERT OR IGNORE INTO user_activity116_traps
         (user_id, activity_id, trap_id) VALUES (?, ?, ?)",
    )
    .bind(user_id)
    .bind(activity_id)
    .bind(trap_id)
    .execute(&mut **tx)
    .await?
    .rows_affected()
        == 1)
}

pub async fn put_trap(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
    trap_id: i32,
) -> Result<bool> {
    Ok(sqlx::query(
        "UPDATE user_activity116_state SET put_trap = ?, updated_at = ?
         WHERE user_id = ? AND activity_id = ?
           AND EXISTS (
               SELECT 1 FROM user_activity116_traps
               WHERE user_id = ? AND activity_id = ? AND trap_id = ?
           )",
    )
    .bind(trap_id)
    .bind(common::time::ServerTime::now_ms())
    .bind(user_id)
    .bind(activity_id)
    .bind(user_id)
    .bind(activity_id)
    .bind(trap_id)
    .execute(pool)
    .await?
    .rows_affected()
        == 1)
}
