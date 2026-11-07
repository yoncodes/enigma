use anyhow::Result;
use sqlx::SqlitePool;

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
