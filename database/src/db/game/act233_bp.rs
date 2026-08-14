use anyhow::Result;
use sqlx::SqlitePool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Act233BpState {
    pub score: i32,
    pub pay_status: i32,
    pub has_get_free_bonus: Vec<i32>,
    pub has_get_pay_bonus: Vec<i32>,
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
}
