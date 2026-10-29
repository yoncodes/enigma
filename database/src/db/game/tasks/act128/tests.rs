use super::*;
use sqlx::SqlitePool;

#[tokio::test]
async fn score_listeners_use_their_owned_dimension() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
    crate::run_migrations(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (1, 'act128-task-score', 0, 0)",
    )
    .execute(&db)
    .await
    .unwrap();

    let mut tx = db.begin().await.unwrap();
    let total_tasks = sync_score(&mut tx, 1, 12113, 1, 40_000, 10_000)
        .await
        .unwrap();
    let special_tasks = sync_score(&mut tx, 1, 130505, 1, 40_000, 10_000)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    assert!(
        total_tasks
            .iter()
            .any(|task| task.task_id == 220301 && task.progress == 25_000 && task.has_finished)
    );
    assert!(
        special_tasks
            .iter()
            .any(|task| task.task_id == 221100 && task.progress == 10_000 && !task.has_finished)
    );
}
