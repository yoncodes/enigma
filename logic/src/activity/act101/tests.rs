use super::*;

#[tokio::test]
async fn bulk_claim_grants_each_day_once() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let rows = &config::configs::get().activity101;
    let activity_id = rows.iter().next().unwrap().activity_id;
    let mut day_ids = rows
        .iter()
        .filter(|row| row.activity_id == activity_id)
        .map(|row| row.id as u32)
        .take(2)
        .collect::<Vec<_>>();
    assert_eq!(day_ids.len(), 2);

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (7, 'act101-list', 0, 0);
             INSERT INTO user_sign_in_info
                (user_id, addup_sign_in_day, open_function_time, reward_mark)
             VALUES (7, 99, 0, 0);",
    )
    .execute(&pool)
    .await
    .unwrap();
    for server_day in 1..=*day_ids.iter().max().unwrap() as i32 {
        activity101::record_login_progress(&pool, 7, &[activity_id], i64::from(server_day))
            .await
            .unwrap();
    }

    day_ids.push(day_ids[0]);
    let claim = get101_bonus_list(&pool, 7, Some(activity_id), day_ids.clone())
        .await
        .unwrap();
    day_ids.sort_unstable();
    day_ids.dedup();
    assert_eq!(claim.reply.ids, day_ids);
    assert!(claim.rewards.is_some());
    assert!(!claim.material_changes.is_empty());

    let retry = get101_bonus_list(&pool, 7, Some(activity_id), day_ids)
        .await
        .unwrap();
    assert!(retry.reply.ids.is_empty());
    assert!(retry.rewards.is_none());
    assert!(retry.material_changes.is_empty());
}

#[tokio::test]
async fn stored_claimable_day_transitions_once() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (7, 'act101', 0, 0);
             INSERT INTO user_sign_in_info
                (user_id, addup_sign_in_day, open_function_time, reward_mark)
             VALUES (7, 99, 0, 0);
             INSERT INTO user_activity_state
                (user_id, activity_id, kind, entry_id, state, progress, ext, updated_at)
             VALUES (7, 101, ?, 1, 1, 0, '', 0);",
    )
    .bind(ActivityStateKind::Act101Day.id())
    .execute(&pool)
    .await
    .unwrap();
    activity101::record_login_progress(&pool, 7, &[101], 1)
        .await
        .unwrap();

    let mut tx = pool.begin().await.unwrap();
    let claimed = activity101::claim_activity101_day_in_transaction(&mut tx, 7, 101, 1)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let mut retry = pool.begin().await.unwrap();
    let claimed_again = activity101::claim_activity101_day_in_transaction(&mut retry, 7, 101, 1)
        .await
        .unwrap();
    retry.commit().await.unwrap();

    assert!(claimed);
    assert!(!claimed_again);
}

#[tokio::test]
async fn login_progress_is_per_activity_and_ignores_global_check_in() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let mut activity_ids = config::configs::get()
        .activity101
        .iter()
        .map(|row| row.activity_id)
        .collect::<Vec<_>>();
    activity_ids.sort_unstable();
    activity_ids.dedup();
    let first = activity_ids
        .iter()
        .copied()
        .find(|activity_id| {
            config::configs::get()
                .activity101
                .iter()
                .filter(|row| row.activity_id == *activity_id)
                .count()
                >= 2
        })
        .unwrap();
    let second = activity_ids
        .into_iter()
        .find(|activity_id| *activity_id != first)
        .unwrap();

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (8, 'act101-login', 0, 0);
             INSERT INTO user_sign_in_info
                (user_id, addup_sign_in_day, open_function_time, reward_mark)
             VALUES (8, 8, 0, 0);",
    )
    .execute(&pool)
    .await
    .unwrap();

    activity101::record_login_progress(&pool, 8, &[first, second], 100)
        .await
        .unwrap();
    activity101::record_login_progress(&pool, 8, &[first, second], 100)
        .await
        .unwrap();
    activity101::record_login_progress(&pool, 8, &[second], 101)
        .await
        .unwrap();
    activity101::record_login_progress(&pool, 8, &[second], 102)
        .await
        .unwrap();
    activity101::record_login_progress(&pool, 8, &[second], 101)
        .await
        .unwrap();

    assert_eq!(
        activity101::get_activity101_info(&pool, 8, first)
            .await
            .unwrap()
            .1,
        1
    );
    assert_eq!(
        activity101::get_activity101_info(&pool, 8, second)
            .await
            .unwrap()
            .1,
        3
    );

    let mut tx = pool.begin().await.unwrap();
    assert!(
        !activity101::claim_activity101_day_in_transaction(&mut tx, 8, first, 2)
            .await
            .unwrap()
    );
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn manager_sync_advances_active_events_once_per_server_day() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let now = super::super::schedule::get(13805).unwrap().start_time;
    let activity_id = active_act101_activity_ids_at(now)
        .into_iter()
        .find(|activity_id| {
            config::configs::get()
                .activity101
                .iter()
                .filter(|row| row.activity_id == *activity_id)
                .count()
                >= 2
        })
        .unwrap();

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (9, 'act101-manager-login', 0, 0);",
    )
    .execute(&pool)
    .await
    .unwrap();

    let manager = ActivityManager::new(9);
    manager
        .sync_act101_login_progress(&pool, now as i64)
        .await
        .unwrap();
    manager
        .sync_act101_login_progress(&pool, now as i64 + 1_000)
        .await
        .unwrap();
    manager
        .sync_act101_login_progress(&pool, now as i64 + 86_400_000)
        .await
        .unwrap();

    assert_eq!(
        activity101::get_activity101_info(&pool, 9, activity_id)
            .await
            .unwrap()
            .1,
        2
    );
}
