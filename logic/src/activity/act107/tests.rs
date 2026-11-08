use super::*;
use sqlx::sqlite::SqlitePoolOptions;

async fn test_pool() -> SqlitePool {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (1, 'act107-store', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}

#[tokio::test]
async fn goods_info_projects_catalog_and_persisted_counts() {
    let pool = test_pool().await;
    let initial = act107_goods_info(&pool, 1, Some(13903)).await.unwrap();
    assert_eq!(initial.activity_id, Some(13903));
    assert_eq!(initial.goods_infos.len(), 32);
    assert!(
        initial
            .goods_infos
            .iter()
            .all(|goods| goods.buy_count == Some(0))
    );

    activity_state::set(
        &pool,
        1,
        13903,
        ActivityStateSet {
            kind: ActivityStateKind::Act107Goods,
            entry_id: 201,
            state: 0,
            progress: 3,
            ext: "",
        },
    )
    .await
    .unwrap();

    let persisted = act107_goods_info(&pool, 1, Some(13903)).await.unwrap();
    assert_eq!(
        persisted
            .goods_infos
            .iter()
            .find(|goods| goods.id == Some(201))
            .and_then(|goods| goods.buy_count),
        Some(3)
    );
}

#[tokio::test]
async fn goods_info_rejects_unknown_store() {
    let pool = test_pool().await;
    let wrong_activity = config::configs::get()
        .activity
        .iter()
        .find(|activity| activity.type_id != ACTIVITY_TYPE_ID)
        .unwrap()
        .id;

    assert!(matches!(
        act107_goods_info(&pool, 1, Some(wrong_activity)).await,
        Err(AppError::InvalidRequest)
    ));
    assert!(matches!(
        act107_goods_info(&pool, 1, None).await,
        Err(AppError::InvalidRequest)
    ));
}
