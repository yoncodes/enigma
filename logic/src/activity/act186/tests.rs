use super::{tasks::sync_at, *};

#[test]
fn parses_act186_weighted_game_type() {
    assert_eq!(first_weighted_id("1#500|2#500"), Some(1));
    assert_eq!(first_weighted_id(""), None);
}

#[test]
fn act186_expiry_uses_task_loop_and_server_time() {
    let now = 1_784_829_600_000;
    let activity_end = parse_time_millis("2026-08-13 04:59:59");

    assert_eq!(activity_end, 1_786_615_199_000);
    assert_eq!(task_expire_time(1, activity_end, now), 1_784_887_200_000);
    assert_eq!(task_expire_time(2, activity_end, now), 1_785_146_400_000);
    assert_eq!(task_expire_time(3, activity_end, now), activity_end);
}

#[tokio::test]
async fn task_events_update_only_the_active_activity() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let now = parse_time_millis("2026-11-06 12:00:00");
    let active = tables
        .actvity186_task
        .iter()
        .find(|row| {
            row.is_online != 0
                && row.activity_id == 13910
                && row.listener_type == "CurrencyDec"
                && row.listener_param == "4"
        })
        .unwrap();
    let expired = tables
        .actvity186_task
        .iter()
        .find(|row| {
            row.is_online != 0
                && row.activity_id == 138504
                && row.listener_type == "CurrencyDec"
                && row.listener_param == "4"
        })
        .unwrap();

    let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (7, 'act186-bulk', 0, 0)",
    )
    .execute(&db)
    .await
    .unwrap();
    let pushes = sync_at(
        &db,
        7,
        crate::task::TaskEvent::CurrencyDec {
            currency_id: 4,
            amount: active.max_progress,
        },
        now,
    )
    .await
    .unwrap();
    assert!(pushes.iter().all(|push| push.activity_id == Some(13910)));
    assert!(pushes[0].act186_tasks.iter().any(|task| {
        task.task_id == Some(active.id) && task.progress == Some(active.max_progress)
    }));
    let state = activity_state::get(&db, 7, 13910, ActivityStateKind::Act186Task)
        .await
        .unwrap();
    assert_eq!(
        state.get(&active.id).map(|value| (value.0, value.1)),
        Some((0, active.max_progress))
    );
    assert!(
        activity_state::get(&db, 7, expired.activity_id, ActivityStateKind::Act186Task)
            .await
            .unwrap()
            .get(&expired.id)
            .is_none()
    );
}

#[tokio::test]
async fn login_progress_resets_daily_and_keeps_cumulative_tasks() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let now = parse_time_millis("2026-11-06 12:00:00");
    let daily = tables
        .actvity186_task
        .iter()
        .find(|row| {
            row.activity_id == 13910 && row.listener_type == "LoginDays" && row.loop_type == 1
        })
        .unwrap();
    let cumulative = tables
        .actvity186_task
        .iter()
        .find(|row| {
            row.activity_id == 13910 && row.listener_type == "LoginDays" && row.loop_type == 3
        })
        .unwrap();
    let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (8, 'act186-login', 0, 0)",
    )
    .execute(&db)
    .await
    .unwrap();

    sync_login(&db, 8, false, false, now).await.unwrap();
    sync_login(&db, 8, true, false, now).await.unwrap();
    let states = activity_state::get(&db, 8, 13910, ActivityStateKind::Act186Task)
        .await
        .unwrap();
    assert_eq!(states[&daily.id].1, 1);
    assert_eq!(states[&cumulative.id].1, 2.min(cumulative.max_progress));

    activity_state::set(
        &db,
        8,
        13910,
        ActivityStateSet {
            kind: ActivityStateKind::Act186Task,
            entry_id: daily.id,
            state: 1,
            progress: daily.max_progress,
            ext: "",
        },
    )
    .await
    .unwrap();
    sync_login(&db, 8, true, false, now).await.unwrap();
    let states = activity_state::get(&db, 8, 13910, ActivityStateKind::Act186Task)
        .await
        .unwrap();
    assert_eq!(states[&daily.id].0, 0);
    assert_eq!(states[&daily.id].1, 1);
}
