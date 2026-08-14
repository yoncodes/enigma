use anyhow::Result;
use sqlx::SqlitePool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity236State {
    pub score: i32,
    pub gain_reward_ids: Vec<i32>,
}

pub async fn get_or_create_state(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
) -> Result<Activity236State> {
    sqlx::query(
        "INSERT OR IGNORE INTO user_activity236_state (user_id, activity_id)
         VALUES (?, ?)",
    )
    .bind(user_id)
    .bind(activity_id)
    .execute(pool)
    .await?;

    get_state(pool, user_id, activity_id).await
}

pub async fn get_state(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
) -> Result<Activity236State> {
    let (score, reward_ids) = sqlx::query_as::<_, (i32, String)>(
        "SELECT score, gain_reward_ids
         FROM user_activity236_state
         WHERE user_id = ? AND activity_id = ?",
    )
    .bind(user_id)
    .bind(activity_id)
    .fetch_one(pool)
    .await?;

    Ok(Activity236State {
        score,
        gain_reward_ids: serde_json::from_str(&reward_ids)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn state_is_isolated_and_persists_by_user_and_activity() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (1, 'act236-a', 0, 0), (2, 'act236-b', 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let initial = get_or_create_state(&pool, 1, 4001).await.unwrap();
        assert_eq!(
            initial,
            Activity236State {
                score: 0,
                gain_reward_ids: Vec::new(),
            }
        );

        sqlx::query(
            "UPDATE user_activity236_state
             SET score = 240, gain_reward_ids = '[3,7]'
             WHERE user_id = 1 AND activity_id = 4001",
        )
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(
            get_state(&pool, 1, 4001).await.unwrap(),
            Activity236State {
                score: 240,
                gain_reward_ids: vec![3, 7],
            }
        );
        assert_eq!(get_or_create_state(&pool, 1, 4002).await.unwrap().score, 0);
        assert_eq!(
            get_or_create_state(&pool, 2, 4001)
                .await
                .unwrap()
                .gain_reward_ids,
            Vec::<i32>::new()
        );
    }
}
