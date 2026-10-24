use super::*;
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::test]
async fn claimed_task_is_persisted_as_no_longer_claimable() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (1, 'task', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_tasks
         (user_id, type_id, task_id, progress, has_finished, finish_count)
         VALUES (1, 1, 40100, 1, 1, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let task = task_db::get_by_id(&pool, 1, 40100).await.unwrap().unwrap();
    let mut tx = pool.begin().await.unwrap();
    let claimed = task_db::finish_task_in_transaction(&mut tx, &task)
        .await
        .unwrap()
        .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(claimed.finish_count, 1);
    assert!(!claimed.has_finished);
    let stored = task_db::get_by_id(&pool, 1, 40100).await.unwrap().unwrap();
    assert_eq!(stored.finish_count, 1);
    assert!(!stored.has_finished);

    task_db::sync_progress(&pool, 1, 1, 40100, 1, 1)
        .await
        .unwrap();
    let synced = task_db::get_by_id(&pool, 1, 40100).await.unwrap().unwrap();
    assert!(!synced.has_finished);
}

#[tokio::test]
async fn task_completion_claims_reached_activity_milestone() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (1, 'task', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_tasks
         (user_id, type_id, task_id, progress, has_finished, finish_count)
         VALUES (1, 2, 20052, 5, 1, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_task_activity
         (user_id, type_id, define_id, value, gain_value, expiry_time)
         VALUES (1, 2, 7, 25, 23, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let claim = TaskManager::new(1).finish(&pool, 20052).await.unwrap();
    let activity = &claim.activity_info[0];

    assert!(claim.act233_bp_scores.is_empty());
    assert_eq!(activity.define_id, 8);
    assert_eq!(activity.value, 27);
    assert_eq!(activity.gain_value, Some(27));
    assert_eq!(claim.rewards.currency_ids, vec![(10, 90)]);
    assert_eq!(claim.rewards.power_item_ids, vec![11]);
}

#[tokio::test]
async fn act233_task_claims_settle_exact_score_and_keep_read_unsupported() {
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
         VALUES (1, 'act233-claim-guard', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_act233_bp_state
         (user_id, activity_id, bp_id, score)
         VALUES (1, 13716, 1, 100)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_tasks
         (user_id, type_id, task_id, progress, has_finished, finish_count, activity_id, updated_at)
         VALUES (1, 79, 790001, 1, 1, 0, 13716, 123),
                (1, 79, 790002, 2, 1, 0, 13716, 123),
                (1, 79, 790003, 0, 0, 0, 13716, 123),
                (1, 79, 790009, 10, 1, 0, 13716, 123)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let mut tasks = TaskManager::new(1);
    let first = tasks.finish(&pool, 790001).await.unwrap();
    assert_eq!(first.reply.finish_count, Some(1));
    assert_eq!(
        first.act233_bp_scores,
        vec![database::db::game::act233_bp::Act233BpScoreUpdate {
            activity_id: 13716,
            bp_id: 1,
            score: 200,
        }]
    );
    assert_eq!(
        database::db::game::act233_bp::get_state(&pool, 1, 13716, 1)
            .await
            .unwrap()
            .score,
        200
    );

    let all = tasks
        .finish_all(&pool, TaskType::ActBp.id(), None, vec![790009], Some(13716))
        .await
        .unwrap();
    assert_eq!(all.task_info[0].finish_count, Some(1));
    assert_eq!(all.act233_bp_scores[0].score, 400);
    assert_eq!(
        database::db::game::act233_bp::get_state(&pool, 1, 13716, 1)
            .await
            .unwrap()
            .score,
        400
    );

    assert!(matches!(
        tasks.finish(&pool, 790009).await,
        Err(AppError::InvalidRequest)
    ));
    assert_eq!(
        database::db::game::act233_bp::get_state(&pool, 1, 13716, 1)
            .await
            .unwrap()
            .score,
        400
    );
    assert!(matches!(
        tasks.finish_read(&pool, Some(790003)).await,
        Err(AppError::InvalidRequest)
    ));

    let stored = task_db::get_by_id(&pool, 1, 790001).await.unwrap().unwrap();
    assert!(!stored.has_finished);
    assert_eq!(stored.finish_count, 1);
    let untouched = task_db::get_by_id(&pool, 1, 790002).await.unwrap().unwrap();
    assert!(untouched.has_finished);
    assert_eq!(untouched.finish_count, 0);
    let second = task_db::get_by_id(&pool, 1, 790009).await.unwrap().unwrap();
    assert!(!second.has_finished);
    assert_eq!(second.finish_count, 1);
    let unread = task_db::get_by_id(&pool, 1, 790003).await.unwrap().unwrap();
    assert_eq!(unread.progress, 0);
    assert!(!unread.has_finished);
    assert_eq!(unread.finish_count, 0);
    assert_eq!(unread.updated_at, 123);
}
