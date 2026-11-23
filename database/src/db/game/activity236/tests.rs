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

#[tokio::test]
async fn score_increment_preserves_claims_and_rolls_back_with_transaction() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    crate::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (3, 'act236-score', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO user_activity236_state
             (user_id, activity_id, score, gain_reward_ids)
             VALUES (3, 13728, 4880, '[1]')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let mut tx = pool.begin().await.unwrap();
    assert_eq!(
        increment_score_in_transaction(&mut tx, 3, 13728, 3280)
            .await
            .unwrap(),
        Activity236State {
            score: 8160,
            gain_reward_ids: vec![1],
        }
    );
    tx.rollback().await.unwrap();

    assert_eq!(
        get_state(&pool, 3, 13728).await.unwrap(),
        Activity236State {
            score: 4880,
            gain_reward_ids: vec![1],
        }
    );
}
