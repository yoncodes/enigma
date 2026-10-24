use super::*;
use crate::player::battle::ActiveBattle;
use database::db::game::tower as tower_db;
use sqlx::sqlite::SqlitePoolOptions;

#[test]
fn mop_up_uses_the_highest_unlocked_reward_tier() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    assert_eq!(
        battle::mop_up_reward(config::configs::get(), 49),
        Some("1#620102#5|1#620101#75")
    );
}

#[tokio::test]
async fn custom_talent_plan_uses_config_costs_and_persists_the_selection() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let root = tables
        .tower_assist_talent
        .iter()
        .find(|row| row.start_node == 1)
        .unwrap();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (16, 'talent', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::starter_data::load_all_starter_data(&pool, 16)
        .await
        .unwrap();

    change_talent_plan(&pool, 16, root.boss_id, 1)
        .await
        .unwrap();
    let before = tower_db::active_talent_plan(&pool, 16, root.boss_id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        !tower_db::activate_talent(
            &pool,
            16,
            root.boss_id,
            before.plan_id,
            root.node_id,
            before.talent_point + 1,
            before.talent_point - root.consume,
        )
        .await
        .unwrap()
    );
    let remaining = activate_talent(&pool, 16, root.boss_id, root.node_id)
        .await
        .unwrap();
    assert_eq!(remaining, before.talent_point - root.consume);
    let restored = reset_talent(&pool, 16, root.boss_id, root.node_id)
        .await
        .unwrap();
    assert_eq!(restored, before.talent_point);
}

#[tokio::test]
async fn fresh_state_comes_from_config_without_live_account_progress() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (15, 'tower', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::starter_data::load_all_starter_data(&pool, 15)
        .await
        .unwrap();

    let reply = info(&pool, 15).await.unwrap();
    assert_eq!(reply.mop_up_times, Some(8));
    assert!(reply.towers.iter().all(|tower| {
        tower.pass_layer_id == Some(0)
            && tower.history_high_score == Some(0)
            && tower.layer_n_os.is_empty()
    }));
    assert!(reply.assist_bosses.iter().all(|boss| {
        boss.level == Some(1)
            && boss.talent_plans.len() == 4
            && boss.use_talent_plan.unwrap_or_default() > 4
    }));
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT COUNT(*) FROM user_tower_opens")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert!(!reply.tower_opens.is_empty());
}

#[tokio::test]
async fn abort_finish_reports_the_saved_layer_and_battle_levels() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (17, 'finish', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::starter_data::load_all_starter_data(&pool, 17)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO user_tower_layers
         (user_id, tower_type, tower_id, layer_id) VALUES (17, 1, 0, 25)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let push = abort_finish_push(
        &pool,
        17,
        &ActiveBattle {
            tower_type: Some(1),
            tower_id: Some(0),
            layer_id: Some(25),
            difficulty: Some(0),
            team_level: Some(180),
            assist_boss_level: Some(10),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(push.score, Some(0));
    assert_eq!(push.boss_level, Some(10));
    assert_eq!(push.team_level, Some(180));
    assert_eq!(push.layer.unwrap().layer_id, Some(25));
}
