use super::*;
use crate::db::game::weekwalk::{WeekwalkEncounterSeed, WeekwalkMapSeed, ensure_cycle};
use sqlx::sqlite::SqlitePoolOptions;

#[test]
fn invalid_weekwalk_listener_config_is_rejected() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    for (listener, params) in [
        ("WeekwalkStar", "layer"),
        ("WeekwalkBattle", "11#3"),
        ("UnknownWeekwalkListener", "1"),
    ] {
        assert!(task_progress(listener, params, &[], config::configs::get()).is_err());
    }
    assert!(period_applies("15", Some(15)).is_err());
    assert!(period_applies("16#15", Some(15)).is_err());
}

#[tokio::test]
async fn earned_stars_complete_weekwalk_reward_thresholds() {
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
         VALUES (1, 'weekwalk-task', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    ensure_cycle(
        &pool,
        1,
        59,
        0,
        i32::MAX,
        &WeekwalkMapSeed {
            map_id: 101,
            scene_id: 101,
            layer: 1,
            is_deep: false,
            buff_id: 0,
            encounters: vec![WeekwalkEncounterSeed {
                battle_id: 800101,
                element_id: 10112,
                visible: true,
            }],
        },
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE user_weekwalk_battles SET star = 2, max_star = 2
         WHERE user_id = 1 AND map_id = 101 AND battle_id = 800101",
    )
    .execute(&pool)
    .await
    .unwrap();

    let updated = sync_weekwalk_tasks(&pool, 1).await.unwrap();
    let threshold = updated.iter().find(|task| task.task_id == 73011).unwrap();
    assert_eq!(threshold.progress, 2);
    assert!(threshold.has_finished);

    let weekly = updated.iter().find(|task| task.task_id == 71001).unwrap();
    assert_eq!(weekly.progress, 2);
    assert!(!weekly.has_finished);

    let deep = config::configs::get()
        .weekwalk
        .iter()
        .find(|map| map.layer == 11 && map.fight_id_front > 0)
        .unwrap();
    sqlx::query("INSERT INTO user_weekwalk_maps (user_id, map_id, scene_id) VALUES (1, ?, ?)")
        .bind(deep.id)
        .bind(deep.scene_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO user_weekwalk_battles
         (user_id, map_id, battle_id, star, max_star)
         VALUES (1, ?, ?, 3, 3)",
    )
    .bind(deep.id)
    .bind(deep.fight_id_front)
    .execute(&pool)
    .await
    .unwrap();
    let updated = sync_weekwalk_tasks(&pool, 1).await.unwrap();
    let fragment = updated.iter().find(|task| task.task_id == 72101).unwrap();
    assert_eq!(fragment.progress, 1);
    assert!(fragment.has_finished);
    let inactive = super::super::get_by_id(&pool, 1, 72001)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(inactive.progress, 0);
    assert!(!inactive.has_finished);
    assert!(sync_weekwalk_tasks(&pool, 1).await.unwrap().is_empty());
}
