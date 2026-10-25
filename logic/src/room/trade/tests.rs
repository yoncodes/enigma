use super::*;
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::test]
async fn starter_orders_follow_room_order_tables() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (1, 'orders', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::starter_data::load_all_starter_data(&pool, 1)
        .await
        .unwrap();

    let reply = RoomManager::new(1)
        .order_info(&pool, config::configs::get())
        .await
        .unwrap();
    let level = config::configs::get()
        .room_order_refresh
        .iter()
        .min_by_key(|row| row.level)
        .unwrap();
    assert_eq!(reply.purchase_order_infos.len(), 4);
    assert_eq!(
        reply.wholesale_order_infos.len(),
        level.meanwhile_wholesale_num as usize
    );
    assert_eq!(reply.remain_refresh_count, Some(-1));
    assert!(reply.purchase_order_infos.iter().all(|order| {
        order.quality == Some(level.level)
            && order.refresh_type == Some(1)
            && order.goods_info.len() == 2
    }));
}
