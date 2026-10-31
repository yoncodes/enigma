use super::*;

#[test]
fn hero_story_times_match_captured_schedule() {
    crate::init_test_server_config();
    let times = schedule::times();
    let configured = common::hero_story_schedule();

    assert_eq!(times.len(), configured.len());
    for (time, row) in times.iter().zip(configured) {
        assert_eq!(time.story_id, Some(row.story_id));
        assert_eq!(time.start_time, Some(row.start_time));
        assert_eq!(time.end_time, Some(row.end_time));
        assert_eq!(time.start_time_resident, Some(row.start_time_resident));
        assert_eq!(time.end_time_resident, Some(row.end_time_resident));
    }
}

#[test]
fn hero_story_active_window_is_inclusive() {
    crate::init_test_server_config();
    let row = &common::hero_story_schedule()[0];

    assert!(schedule::is_active(row.story_id, row.start_time));
    assert!(schedule::is_active(row.story_id, row.end_time));
    assert!(!schedule::is_active(row.story_id, row.start_time - 1));
    assert!(!schedule::is_active(row.story_id, row.end_time + 1));
}

#[tokio::test]
async fn hero_story_reply_includes_configured_times() {
    crate::init_test_server_config();
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (919, 'hero-story-times', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let reply = StoryManager::new(919).hero_story(&pool).await.unwrap();

    assert_eq!(reply.times, schedule::times());
}

#[tokio::test]
async fn hero_story_unlock_consumes_cost_and_rolls_back_on_failure() {
    crate::init_test_server_config();
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    for (player_id, username) in [(920, "hero-story-unlock"), (921, "hero-story-no-cost")] {
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(player_id)
            .bind(username)
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query(
        "INSERT INTO currencies
         (user_id, currency_id, quantity, last_recover_time, expired_time)
         VALUES (920, 24, 2, 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let (reply, consumed) = StoryManager::new(920)
        .unlock_hero_story(&pool, 1)
        .await
        .unwrap();
    assert_eq!(reply.info.unwrap().unlock, Some(true));
    assert_eq!(consumed.currency_ids, [(24, -1)]);
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT quantity FROM currencies WHERE user_id = 920 AND currency_id = 24",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert!(
        StoryManager::new(920)
            .unlock_hero_story(&pool, 1)
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT quantity FROM currencies WHERE user_id = 920 AND currency_id = 24",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );

    assert!(
        StoryManager::new(921)
            .unlock_hero_story(&pool, 1)
            .await
            .is_err()
    );
    assert!(
        !sqlx::query_scalar::<_, bool>(
            "SELECT unlock FROM user_hero_story_state WHERE user_id = 921 AND story_id = 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap()
    );
}

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
