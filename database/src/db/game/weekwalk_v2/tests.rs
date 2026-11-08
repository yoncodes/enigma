use super::*;

async fn test_db() -> SqlitePool {
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    crate::run_migrations(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (1, 'weekwalk-v2-state', 0, 0)",
    )
    .execute(&db)
    .await
    .unwrap();
    db
}

fn seed(time_id: i32) -> WeekwalkV2Seed {
    WeekwalkV2Seed {
        time_id,
        start_time: 100,
        end_time: 200,
        pop_rule: true,
        layers: vec![WeekwalkV2LayerSeed {
            layer_id: 10,
            scene_id: 20,
            unlock: true,
            battles: vec![WeekwalkV2BattleSeed {
                battle_id: 30,
                element_id: 40,
                index: 1,
                visible: true,
            }],
        }],
        previous_layer_ids: vec![9],
    }
}

#[tokio::test]
async fn same_season_read_never_repairs_by_deleting_progress() {
    let db = test_db().await;
    let seed = seed(1);
    ensure_weekwalk_v2_state(&db, 1, &seed).await.unwrap();
    sqlx::query(
        "UPDATE user_weekwalk_v2_layers SET finished = 1, show_finished = 1
         WHERE user_id = 1 AND layer_id = 10",
    )
    .execute(&db)
    .await
    .unwrap();
    sqlx::query(
        "DELETE FROM user_weekwalk_v2_battles
         WHERE user_id = 1 AND layer_id = 10 AND battle_id = 30",
    )
    .execute(&db)
    .await
    .unwrap();

    ensure_weekwalk_v2_state(&db, 1, &seed).await.unwrap();

    let progress: (bool, bool) = sqlx::query_as(
        "SELECT finished, show_finished FROM user_weekwalk_v2_layers
         WHERE user_id = 1 AND layer_id = 10",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(progress, (true, true));
    let battle_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM user_weekwalk_v2_battles WHERE user_id = 1")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(battle_count, 0);
}

#[tokio::test]
async fn rollover_fails_without_erasing_the_previous_season() {
    let db = test_db().await;
    ensure_weekwalk_v2_state(&db, 1, &seed(1)).await.unwrap();
    sqlx::query(
        "UPDATE user_weekwalk_v2_layers SET finished = 1
         WHERE user_id = 1 AND layer_id = 10",
    )
    .execute(&db)
    .await
    .unwrap();

    let error = ensure_weekwalk_v2_state(&db, 1, &seed(2))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("requires settlement"));

    let time_id: i32 =
        sqlx::query_scalar("SELECT time_id FROM user_weekwalk_v2_info WHERE user_id = 1")
            .fetch_one(&db)
            .await
            .unwrap();
    let finished: bool = sqlx::query_scalar(
        "SELECT finished FROM user_weekwalk_v2_layers
         WHERE user_id = 1 AND layer_id = 10",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(time_id, 1);
    assert!(finished);
}
