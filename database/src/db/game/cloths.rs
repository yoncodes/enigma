use sonettobuf::PlayerCloth;
use sqlx::{Sqlite, SqlitePool, Transaction};

/// Reconcile the chapter 13 story clear with Atonal Sequence ownership.
/// Already-owned cloths retain their existing level and experience.
pub async fn reconcile_atonal_sequence_in_transaction(
    tx: &mut Transaction<'_, Sqlite>, user_id: i64,
) -> sqlx::Result<Option<PlayerCloth>> {
    let result = sqlx::query("INSERT INTO user_cloths (user_id, cloth_id, level, exp) \
        SELECT ?, 7, 1, 0 WHERE EXISTS (SELECT 1 FROM user_dungeons WHERE user_id = ? AND episode_id = 11321 AND star > 0) \
        ON CONFLICT(user_id, cloth_id) DO NOTHING")
        .bind(user_id).bind(user_id).execute(&mut **tx).await?;
    if result.rows_affected() == 0 { return Ok(None); }
    Ok(Some(PlayerCloth { cloth_id: Some(7), level: Some(1), exp: Some(0) }))
}

pub async fn reconcile_atonal_sequence(pool: &SqlitePool, user_id: i64) -> sqlx::Result<Option<PlayerCloth>> {
    let mut tx = pool.begin().await?;
    let update = reconcile_atonal_sequence_in_transaction(&mut tx, user_id).await?;
    tx.commit().await?;
    Ok(update)
}

pub async fn get_all(pool: &SqlitePool, user_id: i64) -> sqlx::Result<Vec<PlayerCloth>> {
    let rows: Vec<(i32, i32, i32)> = sqlx::query_as(
        "SELECT cloth_id, level, exp
         FROM user_cloths
         WHERE user_id = ?
         ORDER BY cloth_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(cloth_id, level, exp)| PlayerCloth {
            cloth_id: Some(cloth_id),
            level: Some(level),
            exp: Some(exp),
        })
        .collect())
}

pub async fn unlock(pool: &SqlitePool, user_id: i64, cloth_id: i32) -> sqlx::Result<PlayerCloth> {
    let mut tx = pool.begin().await?;
    let cloth = unlock_in_transaction(&mut tx, user_id, cloth_id).await?;
    tx.commit().await?;
    Ok(cloth)
}

pub async fn unlock_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    cloth_id: i32,
) -> sqlx::Result<PlayerCloth> {
    sqlx::query(
        "INSERT INTO user_cloths (user_id, cloth_id, level, exp)
         VALUES (?, ?, 1, 0)
         ON CONFLICT(user_id, cloth_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(cloth_id)
    .execute(&mut **tx)
    .await?;

    let (level, exp): (i32, i32) =
        sqlx::query_as("SELECT level, exp FROM user_cloths WHERE user_id = ? AND cloth_id = ?")
            .bind(user_id)
            .bind(cloth_id)
            .fetch_one(&mut **tx)
            .await?;

    Ok(PlayerCloth {
        cloth_id: Some(cloth_id),
        level: Some(level),
        exp: Some(exp),
    })
}

pub async fn add_exp(
    pool: &SqlitePool,
    user_id: i64,
    cloth_id: i32,
    amount: i32,
) -> sqlx::Result<PlayerCloth> {
    let mut tx = pool.begin().await?;
    let cloth = add_exp_in_transaction(&mut tx, user_id, cloth_id, amount).await?;
    tx.commit().await?;
    Ok(cloth)
}

pub async fn add_exp_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    cloth_id: i32,
    amount: i32,
) -> sqlx::Result<PlayerCloth> {
    sqlx::query(
        "INSERT INTO user_cloths (user_id, cloth_id, level, exp)
         VALUES (?, ?, 1, ?)
         ON CONFLICT(user_id, cloth_id) DO UPDATE SET exp = exp + excluded.exp",
    )
    .bind(user_id)
    .bind(cloth_id)
    .bind(amount)
    .execute(&mut **tx)
    .await?;

    let (level, exp): (i32, i32) =
        sqlx::query_as("SELECT level, exp FROM user_cloths WHERE user_id = ? AND cloth_id = ?")
            .bind(user_id)
            .bind(cloth_id)
            .fetch_one(&mut **tx)
            .await?;

    Ok(PlayerCloth {
        cloth_id: Some(cloth_id),
        level: Some(level),
        exp: Some(exp),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn atonal_sequence_is_recovered_only_for_completed_chapter_13_and_keeps_existing_level() {
        let db = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        crate::run_migrations(&db).await.unwrap();
        for user_id in [1,2,3] {
            sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
                .bind(user_id).bind(format!("atonal-{user_id}")).execute(&db).await.unwrap();
        }
        sqlx::query("INSERT INTO user_dungeons (user_id, chapter_id, episode_id, star, challenge_count, has_record, left_return_all_num, today_pass_num, today_total_num, created_at, updated_at) VALUES (1, 113, 11321, 1, 0, 0, 1, 0, 0, 0, 0), (2, 113, 11321, 0, 0, 0, 1, 0, 0, 0, 0), (3, 113, 11321, 2, 0, 0, 1, 0, 0, 0, 0)")
            .execute(&db).await.unwrap();
        sqlx::query("INSERT INTO user_cloths (user_id, cloth_id, level, exp) VALUES (3, 7, 3, 42)")
            .execute(&db).await.unwrap();
        assert_eq!(reconcile_atonal_sequence(&db, 1).await.unwrap().unwrap().cloth_id, Some(7));
        assert!(reconcile_atonal_sequence(&db, 1).await.unwrap().is_none());
        assert!(reconcile_atonal_sequence(&db, 2).await.unwrap().is_none());
        assert!(reconcile_atonal_sequence(&db, 3).await.unwrap().is_none());
        let cloth = get_all(&db, 3).await.unwrap().into_iter().find(|c| c.cloth_id == Some(7)).unwrap();
        assert_eq!((cloth.level, cloth.exp), (Some(3), Some(42)));
    }
}
