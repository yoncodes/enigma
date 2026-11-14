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
async fn buying_goods_spends_the_cost_once_and_persists_the_count() {
    let pool = test_pool().await;
    let buy = || buy_act107_goods(&pool, 1, Some(13903), Some(101), Some(1));
    let count = || async {
        act107_goods_info(&pool, 1, Some(13903))
            .await
            .unwrap()
            .goods_infos
            .into_iter()
            .find(|goods| goods.id == Some(101))
            .and_then(|goods| goods.buy_count)
    };

    assert!(buy().await.is_err());
    assert_eq!(count().await, Some(0));

    sqlx::query("INSERT INTO currencies (user_id, currency_id, quantity) VALUES (1, 3901, 8000)")
        .execute(&pool)
        .await
        .unwrap();
    let purchase = buy().await.unwrap();
    assert_eq!(purchase.reply.activity_id, Some(13903));
    assert_eq!(purchase.reply.num, Some(1));
    assert_eq!(
        purchase.reply.goods_info,
        Some(Act107GoodsInfo {
            id: Some(101),
            buy_count: Some(1),
        })
    );
    assert_eq!(purchase.material_changes, vec![(9, 1575, 1)]);
    assert_eq!(purchase.rewards.equip_uids.len(), 1);
    assert_eq!(count().await, Some(1));
    let left: i64 = sqlx::query_scalar(
        "SELECT quantity FROM currencies WHERE user_id = 1 AND currency_id = 3901",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(left, 0);

    assert!(matches!(buy().await, Err(AppError::InvalidRequest)));
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
