use anyhow::Result;
use sqlx::{Sqlite, SqlitePool, Transaction};

pub async fn claimed_bonus_id(db: &SqlitePool, user_id: i64, milestone_id: i32) -> Result<i32> {
    Ok(sqlx::query_scalar(
        "SELECT get_bonus_id FROM user_milestones
         WHERE user_id = ? AND milestone_id = ?",
    )
    .bind(user_id)
    .bind(milestone_id)
    .fetch_optional(db)
    .await?
    .unwrap_or_default())
}

pub async fn advance_claim(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    milestone_id: i32,
    expected: i32,
    next: i32,
) -> Result<bool> {
    let result = sqlx::query(
        "INSERT INTO user_milestones (user_id, milestone_id, get_bonus_id)
         VALUES (?, ?, ?)
         ON CONFLICT(user_id, milestone_id) DO UPDATE SET
             get_bonus_id = excluded.get_bonus_id
         WHERE user_milestones.get_bonus_id = ?",
    )
    .bind(user_id)
    .bind(milestone_id)
    .bind(next)
    .bind(expected)
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected() == 1)
}
