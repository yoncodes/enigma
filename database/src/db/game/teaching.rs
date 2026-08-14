use anyhow::Result;
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::HashSet;

pub async fn claimed_teaching_ids(pool: &SqlitePool, user_id: i64) -> Result<HashSet<i32>> {
    Ok(sqlx::query_scalar(
        "SELECT teaching_id
         FROM user_teaching_bonus_claims
         WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect())
}

pub async fn claim_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    teaching_id: i32,
) -> Result<bool> {
    Ok(sqlx::query(
        "INSERT INTO user_teaching_bonus_claims (user_id, teaching_id)
         VALUES (?, ?)
         ON CONFLICT(user_id, teaching_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(teaching_id)
    .execute(&mut **tx)
    .await?
    .rows_affected()
        == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn teaching_bonus_claims_are_unique_and_persisted() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        crate::run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (1, 'teaching-claims', 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        assert!(claim_in_transaction(&mut tx, 1, 1000).await.unwrap());
        tx.commit().await.unwrap();

        let mut tx = pool.begin().await.unwrap();
        assert!(!claim_in_transaction(&mut tx, 1, 1000).await.unwrap());
        tx.rollback().await.unwrap();

        assert_eq!(claimed_teaching_ids(&pool, 1).await.unwrap().len(), 1);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM user_teaching_bonus_claims
                 WHERE user_id = 1 AND teaching_id = 1000",
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
    }
}
