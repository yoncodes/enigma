use database::db::game::explore_runtime::{self, MapState};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

async fn pool_with_users() -> SqlitePool {
    let pool = SqlitePoolOptions::new().max_connections(1)
        .connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    for user_id in [900001_i64, 900002] {
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind(format!("isolated-explore-{user_id}"))
            .execute(&pool).await.unwrap();
    }
    pool
}

#[tokio::test]
async fn exploration_state_survives_reload_and_is_isolated_by_user() {
    let pool = pool_with_users().await;
    let layout = config::explore_layout::layout(101).unwrap().unwrap();
    let mut tx = pool.begin().await.unwrap();
    explore_runtime::ensure_profile(&mut tx, 900001).await.unwrap();
    let initial = MapState::new(layout);
    assert_eq!((initial.x, initial.y), (1, 6));
    assert_eq!(initial.objects.len(), 199);
    assert_eq!(initial.objects[&189].status, 3);
    assert!(explore_runtime::insert_map(&mut tx, 900001, &initial).await.unwrap());
    assert!(!explore_runtime::insert_map(&mut tx, 900001, &initial).await.unwrap());
    explore_runtime::set_current_map(&mut tx, 900001, 101).await.unwrap();
    tx.commit().await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    let mut saved = explore_runtime::load_map(&mut tx, 900001, 101).await.unwrap().unwrap();
    assert!(layout.node(1, 5).is_some());
    saved.state.x = 1;
    saved.state.y = 5;
    saved.state.visited_nodes.insert((1, 5));
    saved.state.objects.get_mut(&189).unwrap().status = 7;
    explore_runtime::save_map(&mut tx, 900001, &mut saved).await.unwrap();
    tx.commit().await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    let loaded = explore_runtime::load_map(&mut tx, 900001, 101).await.unwrap().unwrap();
    assert_eq!(loaded.revision, 1);
    assert_eq!((loaded.state.x, loaded.state.y), (1, 5));
    assert!(loaded.state.visited_nodes.contains(&(1, 5)));
    assert_eq!(loaded.state.objects[&189].status, 7);
    assert_eq!(explore_runtime::current_map(&mut tx, 900001).await.unwrap(), Some(101));
    explore_runtime::ensure_profile(&mut tx, 900002).await.unwrap();
    assert!(explore_runtime::load_map(&mut tx, 900002, 101).await.unwrap().is_none());
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn stale_write_and_rollback_cannot_erase_progress() {
    let pool = pool_with_users().await;
    let layout = config::explore_layout::layout(101).unwrap().unwrap();
    let mut tx = pool.begin().await.unwrap();
    explore_runtime::ensure_profile(&mut tx, 900001).await.unwrap();
    explore_runtime::insert_map(&mut tx, 900001, &MapState::new(layout)).await.unwrap();
    let original = explore_runtime::load_map(&mut tx, 900001, 101).await.unwrap().unwrap();
    tx.commit().await.unwrap();

    let mut fresh = original.clone();
    fresh.state.objects.get_mut(&189).unwrap().status = 7;
    let mut tx = pool.begin().await.unwrap();
    explore_runtime::save_map(&mut tx, 900001, &mut fresh).await.unwrap();
    tx.commit().await.unwrap();

    let mut stale = original;
    stale.state.objects.get_mut(&189).unwrap().status = 1;
    let mut tx = pool.begin().await.unwrap();
    assert!(explore_runtime::save_map(&mut tx, 900001, &mut stale).await.is_err());
    tx.rollback().await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    let mut loaded = explore_runtime::load_map(&mut tx, 900001, 101).await.unwrap().unwrap();
    assert_eq!(loaded.state.objects[&189].status, 7);
    loaded.state.map_id = 102;
    assert!(explore_runtime::save_map(&mut tx, 900001, &mut loaded).await.is_err());
    loaded.state.map_id = 101;
    loaded.state.objects.get_mut(&189).unwrap().status = 0;
    explore_runtime::save_map(&mut tx, 900001, &mut loaded).await.unwrap();
    tx.rollback().await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    let loaded = explore_runtime::load_map(&mut tx, 900001, 101).await.unwrap().unwrap();
    assert_eq!(loaded.state.objects[&189].status, 7);
    assert_eq!(loaded.revision, 1);
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn summary_only_legacy_progress_is_not_replaced_with_defaults() {
    let pool = pool_with_users().await;
    sqlx::query("INSERT INTO user_explore_maps (user_id, map_id, gold_coin) VALUES (?, 101, 5)")
        .bind(900001_i64).execute(&pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let error = explore_runtime::ensure_profile(&mut tx, 900001).await.unwrap_err();
    assert!(error.to_string().contains("existing exploration progress"));
    tx.rollback().await.unwrap();
    let gold: i64 = sqlx::query_scalar("SELECT gold_coin FROM user_explore_maps WHERE user_id = ? AND map_id = 101")
        .bind(900001_i64).fetch_one(&pool).await.unwrap();
    assert_eq!(gold, 5);
    let profiles: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_explore_runtime_profiles WHERE user_id = ?")
        .bind(900001_i64).fetch_one(&pool).await.unwrap();
    assert_eq!(profiles, 0);
}

#[tokio::test]
async fn exploration_items_are_transactional_and_belong_to_one_user() {
    let pool = pool_with_users().await;
    let mut tx = pool.begin().await.unwrap();
    explore_runtime::ensure_profile(&mut tx, 900001).await.unwrap();
    explore_runtime::ensure_profile(&mut tx, 900002).await.unwrap();
    let items = explore_runtime::grant_item(&mut tx, 900001, 150002, 2).await.unwrap();
    assert_eq!(items.len(), 2);
    assert_ne!(items[0].uid, items[1].uid);
    assert!(items.iter().all(|item| item.uid.unwrap() >= 1_000_000 && item.uid.unwrap() <= i32::MAX as i64));
    assert!(items.iter().all(|item| item.item_id == Some(150002) && item.status == Some(0)));
    assert!(explore_runtime::grant_item(&mut tx, 900001, -1, 1).await.is_err());
    tx.commit().await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    assert_eq!(explore_runtime::list_items(&mut tx, 900001).await.unwrap().len(), 2);
    assert!(explore_runtime::list_items(&mut tx, 900002).await.unwrap().is_empty());
    explore_runtime::grant_item(&mut tx, 900001, 150002, 1).await.unwrap();
    tx.rollback().await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    assert_eq!(explore_runtime::list_items(&mut tx, 900001).await.unwrap().len(), 2);
    tx.commit().await.unwrap();
}
