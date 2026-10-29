use super::*;
use sqlx::sqlite::SqlitePoolOptions;

async fn pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    crate::run_migrations(&pool).await.unwrap();
    for (user_id, username) in [(1, "first"), (2, "other"), (3, "missing")] {
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id)
            .bind(username)
            .execute(&pool)
            .await
            .unwrap();
    }
    pool
}

fn map(map_id: i32, layer: i32) -> WeekwalkMapSeed {
    WeekwalkMapSeed {
        map_id,
        scene_id: map_id,
        layer,
        front_battle_id: map_id * 10 + 1,
        front_element_id: map_id * 10 + 2,
        rear_battle_id: map_id * 10 + 3,
        rear_element_id: map_id * 10 + 4,
    }
}

async fn fight(pool: &SqlitePool, battle_id: i32) -> i64 {
    crate::db::game::battle::create_fight_instance(
        pool,
        crate::db::game::battle::NewFightInstance {
            user_id: 1,
            episode_id: 1,
            battle_id,
            multiplication: 1,
            entry_cost: "",
            checkpoint: "{}",
            created_at: 0,
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn cycle_selects_front_then_rear_and_persists_next_map() {
    let pool = pool().await;
    let first = map(10, 11);
    let next = map(20, 12);
    ensure_cycle(&pool, 1, 7, 100, 200, first).await.unwrap();

    let front = select_battle(&pool, 1, first.map_id, first.front_element_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(front.battle_id, first.front_battle_id);
    assert!(
        select_battle(&pool, 1, first.map_id, first.rear_element_id)
            .await
            .unwrap()
            .is_none()
    );

    assert!(
        complete_battle(
            &pool,
            1,
            front,
            3,
            fight(&pool, front.battle_id).await,
            first.rear_element_id,
            Some(next),
        )
        .await
        .unwrap()
    );
    let rear = select_battle(&pool, 1, first.map_id, first.rear_element_id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        !complete_battle(&pool, 1, front, 3, 0, first.rear_element_id, Some(next))
            .await
            .unwrap()
    );
    assert!(
        complete_battle(
            &pool,
            1,
            rear,
            2,
            fight(&pool, rear.battle_id).await,
            first.rear_element_id,
            Some(next),
        )
        .await
        .unwrap()
    );

    let (info, maps) = get_weekwalk_info(&pool, 1).await.unwrap();
    assert_eq!(info.max_layer, next.layer);
    assert_eq!(maps.len(), 2);
    assert_eq!(maps[0].is_finished, Some(1));
    assert_eq!(maps[1].id, Some(next.map_id));
    assert!(
        select_battle(&pool, 1, next.map_id, next.front_element_id)
            .await
            .unwrap()
            .is_some()
    );
    assert!(get_weekwalk_info(&pool, 2).await.unwrap().1.is_empty());
}

#[tokio::test]
async fn hidden_rear_cannot_skip_the_front_encounter() {
    let pool = pool().await;
    let first = map(10, 11);
    let next = map(20, 12);
    ensure_cycle(&pool, 1, 7, 100, 200, first).await.unwrap();

    let forged_rear = WeekwalkBattleSelection {
        map_id: first.map_id,
        element_id: first.rear_element_id,
        battle_id: first.rear_battle_id,
    };
    assert!(
        !complete_battle(
            &pool,
            1,
            forged_rear,
            3,
            0,
            first.rear_element_id,
            Some(next),
        )
        .await
        .unwrap()
    );
    let (_, maps) = get_weekwalk_info(&pool, 1).await.unwrap();
    assert_eq!(maps.len(), 1);
    assert_eq!(maps[0].is_finished, Some(0));
}

#[tokio::test]
async fn overlapping_cycle_initialization_keeps_one_current_map() {
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect("sqlite::memory:?cache=shared")
        .await
        .unwrap();
    crate::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (1, 'race', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let first = map(10, 11);
    let (left, right) = tokio::join!(
        ensure_cycle(&pool, 1, 7, 100, 200, first),
        ensure_cycle(&pool, 1, 7, 100, 200, first),
    );
    left.unwrap();
    right.unwrap();
    let map_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM user_weekwalk_maps WHERE user_id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(map_count, 1);
}

#[tokio::test]
async fn expired_cycle_is_replaced_but_current_progress_is_kept() {
    let pool = pool().await;
    let first = map(10, 11);
    ensure_cycle(&pool, 2, 7, 100, 200, map(40, 11))
        .await
        .unwrap();
    ensure_cycle(&pool, 1, 7, 100, 200, first).await.unwrap();
    let selection = select_battle(&pool, 1, first.map_id, first.front_element_id)
        .await
        .unwrap()
        .unwrap();
    complete_battle(
        &pool,
        1,
        selection,
        3,
        fight(&pool, selection.battle_id).await,
        first.rear_element_id,
        None,
    )
    .await
    .unwrap();

    ensure_cycle(&pool, 1, 7, 150, 250, first).await.unwrap();
    assert!(
        select_battle(&pool, 1, first.map_id, first.front_element_id)
            .await
            .unwrap()
            .is_none()
    );
    ensure_cycle(&pool, 1, 8, 250, 350, map(30, 11))
        .await
        .unwrap();
    let (info, maps) = get_weekwalk_info(&pool, 1).await.unwrap();
    assert_eq!(info.issue_id, 8);
    assert_eq!(maps.len(), 1);
    assert_eq!(maps[0].id, Some(30));
    assert_eq!(get_weekwalk_info(&pool, 2).await.unwrap().1.len(), 1);
}

#[tokio::test]
async fn shallow_settlement_ack_is_idempotent_and_player_scoped() {
    let pool = pool().await;
    for user_id in [1, 2] {
        sqlx::query(
            "INSERT INTO user_weekwalk_info (user_id, is_pop_shallow_settle) VALUES (?, TRUE)",
        )
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    }

    mark_pop_shallow_settle(&pool, 1).await.unwrap();
    mark_pop_shallow_settle(&pool, 1).await.unwrap();
    mark_pop_shallow_settle(&pool, 3).await.unwrap();

    assert!(
        !get_weekwalk_info(&pool, 1)
            .await
            .unwrap()
            .0
            .is_pop_shallow_settle
    );
    assert!(
        get_weekwalk_info(&pool, 2)
            .await
            .unwrap()
            .0
            .is_pop_shallow_settle
    );
    assert!(
        !get_weekwalk_info(&pool, 3)
            .await
            .unwrap()
            .0
            .is_pop_shallow_settle
    );
    let missing_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM user_weekwalk_info WHERE user_id = 3")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(missing_rows, 0);
}
