use super::*;

#[tokio::test]
async fn critter_book_derives_unlocks_and_persists_only_preferences() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (26, 'book', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO critters
                (uid, player_id, define_id, create_time, special_skin, created_at, updated_at)
             VALUES (1, 26, 500001, 0, 1, 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let manager = CritterManager::new(26);
    assert!(
        manager
            .set_book_special_skin(&pool, config::configs::get(), 500002, true)
            .await
            .is_err()
    );
    assert!(
        manager
            .set_book_background(&pool, config::configs::get(), 500001, 1)
            .await
            .is_err()
    );
    manager
        .mark_book_read(&pool, config::configs::get(), 500001)
        .await
        .unwrap();
    manager
        .set_book_special_skin(&pool, config::configs::get(), 500001, true)
        .await
        .unwrap();
    let info = manager
        .book_info(&pool, config::configs::get())
        .await
        .unwrap()
        .book_infos;

    assert_eq!(info.len(), 1);
    assert_eq!(info[0].id, Some(500001));
    assert_eq!(info[0].unlock_special_skin, Some(true));
    assert_eq!(info[0].use_special_skin, Some(true));
    assert_eq!(info[0].is_new, Some(false));
}
