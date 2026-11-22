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
    crate::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (1, 'act233-tasks', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}

#[tokio::test]
async fn act_bp_tasks_use_loop_order_and_loop_owned_expiry() {
    let pool = test_pool().await;
    ensure_tasks_for_type(&pool, 1, TaskType::ActBp)
        .await
        .unwrap();

    let tasks = list_act_bp(&pool, 1, 13716).await.unwrap();
    assert_eq!(tasks.len(), 15);
    assert!(tasks.windows(2).all(|pair| {
        (pair[0].min_type_id, pair[0].task_id) <= (pair[1].min_type_id, pair[1].task_id)
    }));
    assert_eq!(
        tasks
            .iter()
            .filter(|task| task.min_type_id == TaskLoopType::Weekly.id())
            .count(),
        8
    );
    assert_eq!(
        tasks
            .iter()
            .filter(|task| task.min_type_id == TaskLoopType::Permanent.id())
            .count(),
        7
    );
    let weekly_expiry = ServerTime::next_weekly_refresh_sec(ServerTime::now_ms());
    assert!(
        tasks
            .iter()
            .filter(|task| task.min_type_id == TaskLoopType::Weekly.id())
            .all(|task| task.expiry_time == weekly_expiry)
    );
    assert!(
        tasks
            .iter()
            .filter(|task| task.min_type_id == TaskLoopType::Permanent.id())
            .all(|task| task.expiry_time == 0)
    );

    sync_login_tasks(&pool, 1, false).await.unwrap();
    let refreshed = list_act_bp(&pool, 1, 13716).await.unwrap();
    assert!(
        refreshed
            .iter()
            .filter(|task| task.min_type_id == 2)
            .all(|task| task.expiry_time == weekly_expiry)
    );
    assert!(
        refreshed
            .iter()
            .filter(|task| task.min_type_id == 3)
            .all(|task| task.expiry_time == 0)
    );

    set_progress(&pool, 1, TaskType::ActBp.id(), 790001, 1, true)
        .await
        .unwrap();
    set_progress(&pool, 1, TaskType::ActBp.id(), 790009, 5, false)
        .await
        .unwrap();
    reset_weekly_tasks(&pool, 1).await.unwrap();

    let reset = list_act_bp(&pool, 1, 13716).await.unwrap();
    let weekly = reset.iter().find(|task| task.task_id == 790001).unwrap();
    assert_eq!(weekly.progress, 0);
    assert!(!weekly.has_finished);
    let permanent = reset.iter().find(|task| task.task_id == 790009).unwrap();
    assert_eq!(permanent.progress, 5);
}

#[tokio::test]
async fn version_activity_tasks_replace_prior_version_rows() {
    let pool = test_pool().await;
    let tables = config::configs::get();
    let current_version = tables
        .copost_version_task
        .iter()
        .map(|task| task.version_id)
        .max()
        .unwrap();
    let old = tables
        .copost_version_task
        .iter()
        .find(|task| task.version_id < current_version)
        .unwrap();
    sqlx::query(
        "INSERT INTO user_tasks
             (user_id, type_id, task_id, progress, activity_id, created_at, updated_at)
             VALUES (1, ?, ?, 1, ?, 0, 0)",
    )
    .bind(TaskType::VersionActivity.id())
    .bind(old.id)
    .bind(old.version_id)
    .execute(&pool)
    .await
    .unwrap();

    ensure_tasks_for_type(&pool, 1, TaskType::VersionActivity)
        .await
        .unwrap();
    let tasks = list_by_types(&pool, 1, vec![TaskType::VersionActivity.id()])
        .await
        .unwrap();
    assert!(!tasks.is_empty());
    assert!(tasks.iter().all(|task| task.activity_id == current_version));
    assert!(!tasks.iter().any(|task| task.task_id == old.id));
    assert!(tasks.iter().any(|task| task.task_id == 912));
}

#[tokio::test]
async fn task_resets_clear_only_matching_activity_aggregates() {
    let pool = test_pool().await;
    sqlx::query(
        "INSERT INTO user_task_activity
             (user_id, type_id, define_id, value, gain_value, expiry_time)
             VALUES
                (1, 1, 7, 25, 20, 101),
                (1, 2, 8, 50, 40, 202),
                (1, 7, 9, 60, 50, 303),
                (1, 79, 10, 70, 60, 404)",
    )
    .execute(&pool)
    .await
    .unwrap();

    reset_daily_tasks(&pool, 1).await.unwrap();
    let activities = list_activity(&pool, 1, Vec::new()).await.unwrap();
    let state = |type_id| {
        let activity = activities
            .iter()
            .find(|activity| activity.type_id == type_id)
            .unwrap();
        (
            activity.define_id,
            activity.value,
            activity.gain_value,
            activity.expiry_time,
        )
    };
    assert_eq!(state(1), (0, 0, 0, 101));
    assert_eq!(state(2), (8, 50, 40, 202));
    assert_eq!(state(7), (9, 60, 50, 303));
    assert_eq!(state(79), (10, 70, 60, 404));

    reset_weekly_tasks(&pool, 1).await.unwrap();
    let activities = list_activity(&pool, 1, Vec::new()).await.unwrap();
    let state = |type_id| {
        let activity = activities
            .iter()
            .find(|activity| activity.type_id == type_id)
            .unwrap();
        (
            activity.define_id,
            activity.value,
            activity.gain_value,
            activity.expiry_time,
        )
    };
    assert_eq!(state(1), (0, 0, 0, 101));
    assert_eq!(state(2), (0, 0, 0, 202));
    assert_eq!(state(7), (9, 60, 50, 303));
    assert_eq!(state(79), (10, 70, 60, 404));

    let sync_time = ServerTime::now_ms();
    let daily_expiry = ServerTime::next_daily_refresh_sec(sync_time);
    let weekly_expiry = ServerTime::next_weekly_refresh_sec(sync_time);
    sync_login_tasks(&pool, 1, false).await.unwrap();
    let activities = list_activity(&pool, 1, Vec::new()).await.unwrap();
    let daily = activities
        .iter()
        .find(|activity| activity.type_id == 1)
        .unwrap();
    let weekly = activities
        .iter()
        .find(|activity| activity.type_id == 2)
        .unwrap();
    assert_eq!((daily.define_id, daily.value, daily.gain_value), (0, 0, 0));
    assert_eq!(
        (weekly.define_id, weekly.value, weekly.gain_value),
        (0, 0, 0)
    );
    assert_eq!(daily.expiry_time, daily_expiry);
    assert_eq!(weekly.expiry_time, weekly_expiry);
}

#[tokio::test]
async fn act_bp_login_days_progression_uses_online_login_config() {
    let pool = test_pool().await;

    let updated = sync_login_tasks(&pool, 1, true).await.unwrap();
    let first = updated.iter().find(|task| task.task_id == 790001).unwrap();
    assert_eq!(first.type_id, TaskType::ActBp.id());
    assert_eq!(first.progress, 1);
    assert!(first.has_finished);
    assert!(!updated.iter().any(|task| task.task_id == 790005));
}

#[test]
fn battle_pass_selection_follows_dated_online_task_windows() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    let before_rollover = ServerTime::config_datetime_sec("2026-08-13 04:59:59").unwrap();
    let at_rollover = ServerTime::config_datetime_sec("2026-08-13 05:00:00").unwrap();

    assert_eq!(current_battle_pass_id_at(before_rollover), Some(28));
    assert_eq!(current_battle_pass_id_at(at_rollover), Some(26));
}

#[test]
fn battle_pass_selection_excludes_blank_only_windows() {
    let candidates = [(26, 13_736, true, true), (28, 138_512, true, false)];

    assert_eq!(pick_current_battle_pass_id(candidates), Some(26));
}
