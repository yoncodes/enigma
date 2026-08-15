use anyhow::Result;
use sqlx::{Sqlite, SqlitePool, Transaction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Act233BpState {
    pub score: i32,
    pub pay_status: i32,
    pub has_get_free_bonus: Vec<i32>,
    pub has_get_pay_bonus: Vec<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Act233BpScoreUpdate {
    pub activity_id: i32,
    pub bp_id: i32,
    pub score: i32,
}

pub async fn get_or_create_state(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
    bp_id: i32,
) -> Result<Act233BpState> {
    sqlx::query(
        "INSERT OR IGNORE INTO user_act233_bp_state
            (user_id, activity_id, bp_id)
         VALUES (?, ?, ?)",
    )
    .bind(user_id)
    .bind(activity_id)
    .bind(bp_id)
    .execute(pool)
    .await?;

    get_state(pool, user_id, activity_id, bp_id).await
}

pub async fn get_state(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
    bp_id: i32,
) -> Result<Act233BpState> {
    let (score, pay_status, free_json, pay_json) = sqlx::query_as::<_, (i32, i32, String, String)>(
        "SELECT score, pay_status, has_get_free_bonus, has_get_pay_bonus
         FROM user_act233_bp_state
         WHERE user_id = ? AND activity_id = ? AND bp_id = ?",
    )
    .bind(user_id)
    .bind(activity_id)
    .bind(bp_id)
    .fetch_one(pool)
    .await?;

    Ok(Act233BpState {
        score,
        pay_status,
        has_get_free_bonus: serde_json::from_str(&free_json)?,
        has_get_pay_bonus: serde_json::from_str(&pay_json)?,
    })
}

pub async fn add_score_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    activity_id: i32,
    bp_id: i32,
    score_delta: i32,
) -> Result<Act233BpScoreUpdate> {
    sqlx::query(
        "INSERT OR IGNORE INTO user_act233_bp_state
            (user_id, activity_id, bp_id)
         VALUES (?, ?, ?)",
    )
    .bind(user_id)
    .bind(activity_id)
    .bind(bp_id)
    .execute(&mut **tx)
    .await?;

    let score = sqlx::query_scalar::<_, i32>(
        "UPDATE user_act233_bp_state
         SET score = score + ?, updated_at = ?
         WHERE user_id = ? AND activity_id = ? AND bp_id = ?
         RETURNING score",
    )
    .bind(score_delta)
    .bind(common::time::ServerTime::now_ms())
    .bind(user_id)
    .bind(activity_id)
    .bind(bp_id)
    .fetch_one(&mut **tx)
    .await?;

    Ok(Act233BpScoreUpdate {
        activity_id,
        bp_id,
        score,
    })
}

pub async fn claim_bonus_levels_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    activity_id: i32,
    bp_id: i32,
    current: &Act233BpState,
    free_levels: &[i32],
    pay_levels: &[i32],
) -> Result<Option<Act233BpState>> {
    let mut state = current.clone();
    extend_unique(&mut state.has_get_free_bonus, free_levels);
    extend_unique(&mut state.has_get_pay_bonus, pay_levels);

    let current_free = serde_json::to_string(&current.has_get_free_bonus)?;
    let current_pay = serde_json::to_string(&current.has_get_pay_bonus)?;
    let result = sqlx::query(
        "UPDATE user_act233_bp_state
         SET has_get_free_bonus = ?, has_get_pay_bonus = ?, updated_at = ?
         WHERE user_id = ? AND activity_id = ? AND bp_id = ?
           AND score = ? AND pay_status = ?
           AND has_get_free_bonus = ? AND has_get_pay_bonus = ?",
    )
    .bind(serde_json::to_string(&state.has_get_free_bonus)?)
    .bind(serde_json::to_string(&state.has_get_pay_bonus)?)
    .bind(common::time::ServerTime::now_ms())
    .bind(user_id)
    .bind(activity_id)
    .bind(bp_id)
    .bind(current.score)
    .bind(current.pay_status)
    .bind(current_free)
    .bind(current_pay)
    .execute(&mut **tx)
    .await?;

    Ok((result.rows_affected() == 1).then_some(state))
}

fn extend_unique(values: &mut Vec<i32>, new_values: &[i32]) {
    for value in new_values {
        if !values.contains(value) {
            values.push(*value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn state_is_isolated_and_persists_by_user_activity_and_pass() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (1, 'act233-a', 0, 0), (2, 'act233-b', 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let initial = get_or_create_state(&pool, 1, 13716, 1).await.unwrap();
        assert_eq!(
            initial,
            Act233BpState {
                score: 0,
                pay_status: 0,
                has_get_free_bonus: Vec::new(),
                has_get_pay_bonus: Vec::new(),
            }
        );

        sqlx::query(
            "UPDATE user_act233_bp_state
             SET score = 400, pay_status = 1,
                 has_get_free_bonus = '[2]', has_get_pay_bonus = '[1]'
             WHERE user_id = 1 AND activity_id = 13716 AND bp_id = 1",
        )
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(
            get_state(&pool, 1, 13716, 1).await.unwrap(),
            Act233BpState {
                score: 400,
                pay_status: 1,
                has_get_free_bonus: vec![2],
                has_get_pay_bonus: vec![1],
            }
        );
        assert_eq!(
            get_or_create_state(&pool, 1, 13717, 2).await.unwrap().score,
            0
        );
        assert_eq!(
            get_or_create_state(&pool, 2, 13716, 1)
                .await
                .unwrap()
                .pay_status,
            0
        );
    }

    #[tokio::test]
    async fn stale_claim_state_cannot_update_twice() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (1, 'act233-cas', 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let stale = get_or_create_state(&pool, 1, 13716, 1).await.unwrap();

        let mut first = pool.begin().await.unwrap();
        let updated = claim_bonus_levels_in_transaction(&mut first, 1, 13716, 1, &stale, &[1], &[])
            .await
            .unwrap();
        assert!(updated.is_some());
        first.commit().await.unwrap();

        let mut second = pool.begin().await.unwrap();
        let duplicate =
            claim_bonus_levels_in_transaction(&mut second, 1, 13716, 1, &stale, &[1], &[])
                .await
                .unwrap();
        assert!(duplicate.is_none());
        second.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn score_update_rolls_back_with_its_transaction() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (1, 'act233-score-tx', 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        let update = add_score_in_transaction(&mut tx, 1, 13716, 1, 100)
            .await
            .unwrap();
        assert_eq!(update.score, 100);
        tx.rollback().await.unwrap();
        assert_eq!(
            get_or_create_state(&pool, 1, 13716, 1).await.unwrap().score,
            0
        );

        let mut tx = pool.begin().await.unwrap();
        let update = add_score_in_transaction(&mut tx, 1, 13716, 1, 100)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(update.score, 100);
        assert_eq!(get_state(&pool, 1, 13716, 1).await.unwrap().score, 100);
    }
}
