use super::*;

const USER_ID: i64 = 12;
const SERVER_DAY_START: i64 = 1_898_762_400_000; // 2030-03-03 10:00:00 UTC

async fn test_pool() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    crate::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (?, 'maker', 0, 0)",
    )
    .bind(USER_ID)
    .execute(&pool)
    .await
    .unwrap();
    pool
}

async fn seed_progress(pool: &SqlitePool, next: i32, updated_at: i64) {
    sqlx::query(
        "INSERT INTO user_power_maker_state
                 (user_id, status, next_remain_second, updated_at)
             VALUES (?, 0, ?, ?)",
    )
    .bind(USER_ID)
    .bind(next)
    .bind(updated_at)
    .execute(pool)
    .await
    .unwrap();
}

async fn settle(pool: &SqlitePool, now: i64, making: bool) {
    let mut tx = pool.begin().await.unwrap();
    settle_in_transaction(&mut tx, USER_ID, now, making, None)
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn production_pauses_and_resumes_without_losing_progress() {
    let pool = test_pool().await;
    seed_progress(&pool, 3_600, SERVER_DAY_START).await;

    settle(&pool, SERVER_DAY_START + 7_200_000, false).await;
    assert_eq!(
        get_state(&pool, USER_ID).await.unwrap(),
        PowerMakerState {
            status: 0,
            next_remain_second: 3_600,
            make_count: 0,
            logout_second: 0,
        }
    );

    settle(&pool, SERVER_DAY_START + 9_000_000, true).await;
    assert_eq!(
        get_state(&pool, USER_ID).await.unwrap().next_remain_second,
        1_800
    );

    settle(&pool, SERVER_DAY_START + 10_800_000, true).await;
    let state = get_state(&pool, USER_ID).await.unwrap();
    assert_eq!(
        (state.status, state.next_remain_second, state.make_count),
        (1, 43_200, 1)
    );
    let items = get_maker_items(&pool, USER_ID).await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!((items[0].item_id, items[0].quantity), (Some(31), Some(1)));
}

#[tokio::test]
async fn active_settlement_preserves_subsecond_progress() {
    let pool = test_pool().await;
    seed_progress(&pool, 10, SERVER_DAY_START).await;

    settle(&pool, SERVER_DAY_START + 900, true).await;
    let first: (i32, i64) = sqlx::query_as(
        "SELECT next_remain_second, updated_at
             FROM user_power_maker_state WHERE user_id = ?",
    )
    .bind(USER_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(first, (10, SERVER_DAY_START));

    settle(&pool, SERVER_DAY_START + 1_100, true).await;
    let second: (i32, i64) = sqlx::query_as(
        "SELECT next_remain_second, updated_at
             FROM user_power_maker_state WHERE user_id = ?",
    )
    .bind(USER_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(second, (9, SERVER_DAY_START + 1_000));
}

#[tokio::test]
async fn production_groups_two_daily_sweets_into_seven_day_expiry_batches() {
    let pool = test_pool().await;
    seed_progress(&pool, 6 * 60 * 60, SERVER_DAY_START).await;

    settle(&pool, SERVER_DAY_START + 66 * 60 * 60 * 1_000, true).await;

    let rows = sqlx::query_as::<_, (i32, i64)>(
        "SELECT quantity, expire_time FROM power_items
             WHERE user_id = ? AND item_id = 31 ORDER BY expire_time",
    )
    .bind(USER_ID)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows.iter().map(|row| row.0).collect::<Vec<_>>(),
        vec![2, 2, 2]
    );
    assert_eq!(
        rows.iter().map(|row| row.1).collect::<Vec<_>>(),
        vec![
            (SERVER_DAY_START + 7 * DAY_MS) / 1_000,
            (SERVER_DAY_START + 8 * DAY_MS) / 1_000,
            (SERVER_DAY_START + 9 * DAY_MS) / 1_000,
        ]
    );
    assert_eq!(
        get_state(&pool, USER_ID).await.unwrap().next_remain_second,
        PRODUCTION_SECONDS
    );
}

#[tokio::test]
async fn login_consumes_pending_offline_summary_once() {
    let pool = test_pool().await;
    sqlx::query(
        "INSERT INTO user_power_maker_state
                 (user_id, status, next_remain_second, make_count, logout_second, updated_at)
             VALUES (?, 0, 1234, 3, 99, ?)",
    )
    .bind(USER_ID)
    .bind(ServerTime::now_ms())
    .execute(&pool)
    .await
    .unwrap();

    let login = take_state(&pool, USER_ID, true).await.unwrap();
    let repeated = take_state(&pool, USER_ID, true).await.unwrap();
    assert_eq!((login.make_count, login.logout_second), (3, 99));
    assert_eq!((repeated.make_count, repeated.logout_second), (0, 0));
}

#[tokio::test]
async fn natural_recovery_hands_elapsed_at_cap_to_the_machine() {
    let pool = test_pool().await;
    let recover_limit = 240;
    let recover_time = 360;
    let recover_num = 1;
    let recover_interval = i64::from(recover_time) * 1_000;
    seed_progress(&pool, PRODUCTION_SECONDS, SERVER_DAY_START).await;
    sqlx::query(
        "INSERT INTO currencies
                 (user_id, currency_id, quantity, last_recover_time, expired_time)
             VALUES (?, 4, ?, ?, 0)",
    )
    .bind(USER_ID)
    .bind(recover_limit - recover_num)
    .bind(SERVER_DAY_START)
    .execute(&pool)
    .await
    .unwrap();

    let now = SERVER_DAY_START + recover_interval + i64::from(PRODUCTION_SECONDS) * 1_000;
    let mut tx = pool.begin().await.unwrap();
    super::super::currencies::settle_loaded_power(
        &mut tx,
        USER_ID,
        now,
        super::super::currencies::PowerRecovery {
            quantity: recover_limit - recover_num,
            last_recover_time: Some(SERVER_DAY_START),
            limit: recover_limit,
            interval_seconds: recover_time,
            amount: recover_num,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(
        get_state(&pool, USER_ID).await.unwrap(),
        PowerMakerState {
            status: 1,
            next_remain_second: PRODUCTION_SECONDS,
            make_count: 1,
            logout_second: 0,
        }
    );
    let quantity: i32 =
        sqlx::query_scalar("SELECT quantity FROM currencies WHERE user_id = ? AND currency_id = 4")
            .bind(USER_ID)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(quantity, recover_limit);
}
