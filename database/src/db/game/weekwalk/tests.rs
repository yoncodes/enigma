use super::*;
use sqlx::sqlite::SqlitePoolOptions;

async fn pool() -> SqlitePool {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
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
        is_deep: layer >= 11,
        buff_id: 0,
        encounters: vec![
            WeekwalkEncounterSeed {
                battle_id: map_id * 10 + 1,
                element_id: map_id * 10 + 2,
                visible: true,
            },
            WeekwalkEncounterSeed {
                battle_id: map_id * 10 + 3,
                element_id: map_id * 10 + 4,
                visible: false,
            },
        ],
    }
}

fn front(map: &WeekwalkMapSeed) -> WeekwalkEncounterSeed {
    map.encounters[0]
}

fn rear(map: &WeekwalkMapSeed) -> WeekwalkEncounterSeed {
    map.encounters[1]
}

fn finished(next_map: Option<WeekwalkMapSeed>) -> WeekwalkElementState {
    WeekwalkElementState {
        current_index: 0,
        next_index: 1,
        is_finish: true,
        next_map,
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
    ensure_cycle(&pool, 1, 7, 100, 200, &first).await.unwrap();

    let front_seed = front(&first);
    let rear_seed = rear(&first);
    let front_selection = select_battle(&pool, 1, first.map_id, front_seed.element_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(front_selection.battle_id, front_seed.battle_id);
    assert!(
        select_battle(&pool, 1, first.map_id, rear_seed.element_id)
            .await
            .unwrap()
            .is_none()
    );

    assert!(
        complete_battle(
            &pool,
            1,
            front_selection,
            3,
            fight(&pool, front_selection.battle_id).await,
            Some(rear_seed.element_id),
            finished(Some(next.clone())),
        )
        .await
        .unwrap()
        .is_some()
    );
    let rear = select_battle(&pool, 1, first.map_id, rear_seed.element_id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        complete_battle(
            &pool,
            1,
            front_selection,
            3,
            0,
            None,
            finished(Some(next.clone()))
        )
        .await
        .unwrap()
        .is_none()
    );
    assert!(
        complete_battle(
            &pool,
            1,
            rear,
            2,
            fight(&pool, rear.battle_id).await,
            None,
            finished(Some(next.clone())),
        )
        .await
        .unwrap()
        .is_some()
    );

    let (info, maps) = get_weekwalk_info(&pool, 1).await.unwrap();
    assert_eq!(info.max_layer, next.layer);
    assert_eq!(maps.len(), 2);
    assert_eq!(maps[0].is_finished, Some(1));
    assert_eq!(maps[1].id, Some(next.map_id));
    assert!(
        select_battle(&pool, 1, next.map_id, front(&next).element_id)
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
    ensure_cycle(&pool, 1, 7, 100, 200, &first).await.unwrap();

    let forged_rear = WeekwalkBattleSelection {
        map_id: first.map_id,
        element_id: rear(&first).element_id,
        battle_id: rear(&first).battle_id,
    };
    assert!(
        complete_battle(&pool, 1, forged_rear, 3, 0, None, finished(Some(next)))
            .await
            .unwrap()
            .is_none()
    );
    let (_, maps) = get_weekwalk_info(&pool, 1).await.unwrap();
    assert_eq!(maps.len(), 1);
    assert_eq!(maps[0].is_finished, Some(0));
}

#[tokio::test]
async fn shallow_map_unlocks_next_after_all_encounters_finish() {
    let pool = pool().await;
    let first = WeekwalkMapSeed {
        map_id: 101,
        scene_id: 101,
        layer: 1,
        is_deep: false,
        buff_id: 0,
        encounters: (1..=5)
            .map(|index| WeekwalkEncounterSeed {
                battle_id: 800_100 + index,
                element_id: 10_110 + index,
                visible: true,
            })
            .collect(),
    };
    let next = WeekwalkMapSeed {
        map_id: 102,
        scene_id: 102,
        layer: 2,
        ..first.clone()
    };
    ensure_cycle(&pool, 1, 7, 100, 200, &first).await.unwrap();

    for (index, encounter) in first.encounters.iter().enumerate() {
        let selection = select_battle(&pool, 1, first.map_id, encounter.element_id)
            .await
            .unwrap()
            .unwrap();
        assert!(
            complete_battle(
                &pool,
                1,
                selection,
                2,
                fight(&pool, selection.battle_id).await,
                None,
                finished(Some(next.clone())),
            )
            .await
            .unwrap()
            .is_some()
        );
        let (info, maps) = get_weekwalk_info(&pool, 1).await.unwrap();
        assert_eq!(maps.len(), if index == 4 { 2 } else { 1 });
        assert_eq!(info.max_layer, if index == 4 { 2 } else { 1 });
    }
}

#[tokio::test]
async fn current_deep_only_state_gains_the_first_shallow_map() {
    let pool = pool().await;
    let deep = map(900, 11);
    ensure_cycle(&pool, 1, 7, 100, 200, &deep).await.unwrap();
    let shallow = WeekwalkMapSeed {
        layer: 1,
        ..map(101, 1)
    };

    ensure_cycle(&pool, 1, 7, 150, 250, &shallow).await.unwrap();

    let (_, maps) = get_weekwalk_info(&pool, 1).await.unwrap();
    assert!(maps.iter().any(|map| map.id == Some(deep.map_id)));
    assert!(maps.iter().any(|map| map.id == Some(shallow.map_id)));

    let first = select_battle(&pool, 1, shallow.map_id, front(&shallow).element_id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        complete_battle(
            &pool,
            1,
            first,
            2,
            fight(&pool, first.battle_id).await,
            Some(rear(&shallow).element_id),
            finished(Some(deep.clone())),
        )
        .await
        .unwrap()
        .is_some()
    );
    let second = select_battle(&pool, 1, shallow.map_id, rear(&shallow).element_id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        complete_battle(
            &pool,
            1,
            second,
            2,
            fight(&pool, second.battle_id).await,
            None,
            finished(Some(deep.clone())),
        )
        .await
        .unwrap()
        .is_some()
    );
    let (info, maps) = get_weekwalk_info(&pool, 1).await.unwrap();
    assert_eq!(info.max_layer, 11);
    assert_eq!(maps.len(), 2);
}

#[tokio::test]
async fn dialog_history_replaces_only_the_selected_players_element() {
    let pool = pool().await;
    let map = map(101, 1);
    ensure_cycle(&pool, 1, 7, 100, 200, &map).await.unwrap();
    ensure_cycle(&pool, 2, 7, 100, 200, &map).await.unwrap();
    let element_id = front(&map).element_id;

    assert!(
        replace_element_history(&pool, 1, map.map_id, element_id, &["first".into()])
            .await
            .unwrap()
    );
    assert!(
        replace_element_history(&pool, 1, map.map_id, element_id, &["replacement".into()])
            .await
            .unwrap()
    );
    assert!(
        replace_element_history(&pool, 2, map.map_id, element_id, &["other".into()])
            .await
            .unwrap()
    );

    let (_, first) = get_weekwalk_info(&pool, 1).await.unwrap();
    let (_, other) = get_weekwalk_info(&pool, 2).await.unwrap();
    assert_eq!(first[0].element_infos[0].historylist, ["replacement"]);
    assert_eq!(other[0].element_infos[0].historylist, ["other"]);
    assert!(first[0].element_infos[1].historylist.is_empty());
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
        ensure_cycle(&pool, 1, 7, 100, 200, &first),
        ensure_cycle(&pool, 1, 7, 100, 200, &first),
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
    ensure_cycle(&pool, 2, 7, 100, 200, &map(40, 11))
        .await
        .unwrap();
    ensure_cycle(&pool, 1, 7, 100, 200, &first).await.unwrap();
    let selection = select_battle(&pool, 1, first.map_id, front(&first).element_id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        complete_battle(
            &pool,
            1,
            selection,
            3,
            fight(&pool, selection.battle_id).await,
            Some(rear(&first).element_id),
            finished(None),
        )
        .await
        .unwrap()
        .is_some()
    );

    ensure_cycle(&pool, 1, 7, 150, 250, &first).await.unwrap();
    assert!(
        select_battle(&pool, 1, first.map_id, front(&first).element_id)
            .await
            .unwrap()
            .is_none()
    );
    ensure_cycle(&pool, 1, 8, 250, 350, &map(30, 11))
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
