use super::*;
use database::db::game::tasks::TaskType;
use sqlx::sqlite::SqlitePoolOptions;

async fn task_reset_pool() -> SqlitePool {
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
         VALUES (91, 'periodic-reset', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}

async fn insert_claimable_task(pool: &SqlitePool, type_id: i32, task_id: i32) {
    sqlx::query(
        "INSERT INTO user_tasks
         (user_id, type_id, task_id, progress, has_finished, finish_count)
         VALUES (91, ?, ?, 99, 1, 1)",
    )
    .bind(type_id)
    .bind(task_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_task_activity
         (user_id, type_id, define_id, value, gain_value)
         VALUES (91, ?, 4, 50, 40)",
    )
    .bind(type_id)
    .execute(pool)
    .await
    .unwrap();
}

#[test]
fn parses_split_token_login() {
    let mut data = Vec::new();
    data.extend_from_slice(&7u16.to_be_bytes());
    data.extend_from_slice(b"1_12345");
    data.extend_from_slice(&3u16.to_be_bytes());
    data.extend_from_slice(b"tok");

    assert_eq!(
        parse_login_request(&data).unwrap(),
        LoginRequest {
            account_id: "1_12345".into(),
            token: "tok".into(),
        }
    );
}

#[test]
fn parses_inline_token_login() {
    let account = b"1_12345#tok";
    let mut data = Vec::new();
    data.extend_from_slice(&(account.len() as u16).to_be_bytes());
    data.extend_from_slice(account);

    assert_eq!(
        parse_login_request(&data).unwrap(),
        LoginRequest {
            account_id: "1_12345".into(),
            token: "tok".into(),
        }
    );
}

#[test]
fn login_reply_matches_live_wire_shape() {
    assert_eq!(
        login_reply_payload(0x17eb591e),
        [0, 0, 0, 0, 0, 0, 0x17, 0xeb, 0x59, 0x1e]
    );
}

#[tokio::test]
async fn periodic_reconciliation_resets_each_boundary_once_and_blocks_stale_claims() {
    const DAY_MS: i64 = 86_400_000;
    const NOW_MS: i64 = 1_786_766_400_000;

    let pool = task_reset_pool().await;
    let tables = config::configs::get();
    let daily_id = tables
        .task_daily
        .iter()
        .find(|task| task.is_online != 0)
        .unwrap()
        .id;
    let weekly_id = tables
        .task_weekly
        .iter()
        .find(|task| task.is_online != 0)
        .unwrap()
        .id;
    insert_claimable_task(&pool, TaskType::Daily.id(), daily_id).await;
    insert_claimable_task(&pool, TaskType::Weekly.id(), weekly_id).await;

    let mut state = crate::player::PlayerState::new(91, NOW_MS);
    state.last_daily_reset_time = Some(NOW_MS - DAY_MS);
    let mut player = Player::new(91, state);

    assert_eq!(
        reconcile_periodic_resets_for_player(&mut player, &pool, NOW_MS)
            .await
            .unwrap(),
        (true, false)
    );
    assert_eq!(player.state.last_daily_reset_time, Some(NOW_MS));
    assert_eq!(player.state.last_weekly_reset_time, Some(NOW_MS));
    assert_eq!(
        sqlx::query_as::<_, (i64, bool, i64)>(
            "SELECT progress, has_finished, finish_count FROM user_tasks
             WHERE user_id = 91 AND type_id = 1 AND task_id = ?",
        )
        .bind(daily_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        (0, false, 0)
    );
    assert_eq!(
        sqlx::query_as::<_, (i64, i64, i64)>(
            "SELECT define_id, value, gain_value FROM user_task_activity
             WHERE user_id = 91 AND type_id = 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        (0, 0, 0)
    );
    assert!(player.tasks.finish(&pool, daily_id).await.is_err());
    assert_eq!(
        reconcile_periodic_resets_for_player(&mut player, &pool, NOW_MS)
            .await
            .unwrap(),
        (false, false)
    );

    player.state.last_weekly_reset_time = Some(NOW_MS - 8 * DAY_MS);
    assert_eq!(
        reconcile_periodic_resets_for_player(&mut player, &pool, NOW_MS)
            .await
            .unwrap(),
        (false, true)
    );
    assert_eq!(player.state.last_weekly_reset_time, Some(NOW_MS));
    assert_eq!(
        sqlx::query_as::<_, (i64, bool, i64)>(
            "SELECT progress, has_finished, finish_count FROM user_tasks
             WHERE user_id = 91 AND type_id = 2 AND task_id = ?",
        )
        .bind(weekly_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        (0, false, 0)
    );
    assert_eq!(
        sqlx::query_as::<_, (i64, i64, i64)>(
            "SELECT define_id, value, gain_value FROM user_task_activity
             WHERE user_id = 91 AND type_id = 2",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        (0, 0, 0)
    );
    assert_eq!(
        reconcile_periodic_resets_for_player(&mut player, &pool, NOW_MS)
            .await
            .unwrap(),
        (false, false)
    );
}

#[tokio::test]
async fn periodic_reconciliation_does_not_advance_markers_when_reset_fails() {
    const NOW_MS: i64 = 1_786_766_400_000;
    let pool = task_reset_pool().await;
    let mut state = crate::player::PlayerState::new(91, NOW_MS);
    state.last_daily_reset_time = Some(NOW_MS - 86_400_000);
    let original = state.last_daily_reset_time;
    let mut player = Player::new(91, state);
    pool.close().await;

    assert!(
        reconcile_periodic_resets_for_player(&mut player, &pool, NOW_MS)
            .await
            .is_err()
    );
    assert_eq!(player.state.last_daily_reset_time, original);
}
