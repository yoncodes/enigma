use super::add_buy_count_in_transaction;
use sqlx::SqlitePool;

#[tokio::test]
async fn advances_absent_and_reset_rows_but_rejects_stale_zero() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(
        "CREATE TABLE user_store_goods (
            user_id INTEGER NOT NULL,
            goods_id INTEGER NOT NULL,
            buy_count INTEGER NOT NULL,
            PRIMARY KEY (user_id, goods_id)
        )",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO user_store_goods VALUES (1, 6100027, 0)")
        .execute(&pool)
        .await
        .unwrap();

    let mut tx = pool.begin().await.unwrap();
    let count = add_buy_count_in_transaction(&mut tx, 1, 6100027, 0, 1)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(count, Some(1));
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT buy_count FROM user_store_goods WHERE user_id = 1 AND goods_id = 6100027",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );

    let mut tx = pool.begin().await.unwrap();
    let count = add_buy_count_in_transaction(&mut tx, 2, 6100027, 0, 1)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(count, Some(1));

    let mut tx = pool.begin().await.unwrap();
    let count = add_buy_count_in_transaction(&mut tx, 1, 6100027, 0, 1)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(count, None);
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT buy_count FROM user_store_goods WHERE user_id = 1 AND goods_id = 6100027",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}
