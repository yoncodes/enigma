use common::time::ServerTime;
use sqlx::{Sqlite, SqlitePool, Transaction};

pub async fn get_claims(pool: &SqlitePool, user_id: i64) -> sqlx::Result<Vec<i32>> {
    sqlx::query_scalar(
        "SELECT invite_id
         FROM user_hero_invitation_claims
         WHERE user_id = ?
         ORDER BY invite_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn claim_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    invite_id: i32,
) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "INSERT INTO user_hero_invitation_claims (user_id, invite_id, claimed_at)
         VALUES (?, ?, ?)
         ON CONFLICT(user_id, invite_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(invite_id)
    .bind(ServerTime::now_ms())
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected() == 1)
}
