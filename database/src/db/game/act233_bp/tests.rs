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
    let duplicate = claim_bonus_levels_in_transaction(&mut second, 1, 13716, 1, &stale, &[1], &[])
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
