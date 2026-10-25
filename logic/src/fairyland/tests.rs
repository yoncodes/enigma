use super::*;

#[tokio::test]
async fn fairyland_progress_uses_config_and_generic_activity_state() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (25, 'fairy', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let manager = FairylandManager::new(25);
    assert!(
        manager
            .resolve_puzzle(&pool, config::configs::get(), 4, "wrong")
            .await
            .is_err()
    );
    manager
        .resolve_puzzle(&pool, config::configs::get(), 4, "6")
        .await
        .unwrap();
    manager
        .record_dialog(&pool, config::configs::get(), 1)
        .await
        .unwrap();
    let reply = manager
        .record_element(&pool, config::configs::get(), 1)
        .await
        .unwrap();
    let info = reply.info.unwrap();

    assert_eq!(info.pass_puzzle_id, vec![4]);
    assert_eq!(info.dialog_id, vec![1]);
    assert_eq!(info.finish_element_id, vec![1]);
}
