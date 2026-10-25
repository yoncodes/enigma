use super::*;

#[tokio::test]
async fn update_story_minus_one_finishes_story() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(
        "CREATE TABLE user_finished_stories (
                user_id INTEGER NOT NULL,
                story_id INTEGER NOT NULL,
                PRIMARY KEY (user_id, story_id)
            )",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE user_processing_stories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                story_id INTEGER NOT NULL,
                step_id INTEGER NOT NULL DEFAULT 0,
                favor INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                UNIQUE(user_id, story_id)
            )",
    )
    .execute(&pool)
    .await
    .unwrap();

    let manager = StoryManager::new(7);
    manager.update(&pool, 1001, 3, 5).await.unwrap();
    assert_eq!(
        manager.finish_state(&pool, 1001).await.unwrap().is_finish,
        Some(false)
    );
    let update = manager.update(&pool, 1001, -1, 0).await.unwrap();

    let processing_count: i32 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM user_processing_stories WHERE user_id = 7 AND story_id = 1001",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let finished_count: i32 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM user_finished_stories WHERE user_id = 7 AND story_id = 1001",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(update.finished_story_id, Some(1001));
    assert_eq!(
        manager.finish_state(&pool, 1001).await.unwrap().is_finish,
        Some(true)
    );
    assert_eq!(processing_count, 0);
    assert_eq!(finished_count, 1);
}
