//! Transactional movement for the exploration primitives implemented so far.

use anyhow::{Context, Result};
use database::db::game::explore_runtime;
use sonettobuf::ExploreStep;
use sqlx::SqlitePool;

use crate::{explore_engine, explore_entry};

pub struct Movement {
    pub steps: Vec<ExploreStep>,
    pub tasks: Vec<database::models::game::tasks::UserTask>,
}

pub async fn move_player(pool: &SqlitePool, user_id: i64, x: i32, y: i32) -> Result<Movement> {
    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user_id).await?
        .context("no current exploration map")?;
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    let mut stored = explore_runtime::load_map(&mut tx, user_id, map_id).await?
        .context("exploration map state is missing")?;
    if (stored.state.x, stored.state.y) == (x, y) {
        return Ok(Movement { steps: Vec::new(), tasks: Vec::new() });
    }
    let selected = explore_runtime::use_item_uid(&mut tx, user_id).await?.context("missing exploration profile")?;
    let carried = crate::explore_pipe::carried_pot(&stored.state, layout, selected)?;
    let changed = explore_engine::move_client_with_ignored_unit(&mut stored.state, layout, x, y, carried)?;
    let mut gold_delta = 0;
    let mut purple_delta = 0;
    for id in &changed {
        let unit = layout.unit(*id).context("changed map object is missing")?;
        if unit.kind == 221 {
            match unit.special_data.first().and_then(|value| value.as_str()) {
                Some("1") => gold_delta += 1,
                Some("2") => purple_delta += 1,
                _ => anyhow::bail!("unknown exploration coin type"),
            }
        }
    }
    let mut totals = None;
    if gold_delta > 0 || purple_delta > 0 {
        explore_entry::ensure_map_summary(&mut tx, user_id, layout).await?;
        sqlx::query("UPDATE user_explore_maps SET gold_coin = gold_coin + ?, purple_coin = purple_coin + ? \
            WHERE user_id = ? AND map_id = ?")
            .bind(gold_delta).bind(purple_delta).bind(user_id).bind(map_id).execute(&mut *tx).await?;
        totals = Some(sqlx::query_as::<_, (i32, i32)>(
            "SELECT gold_coin, purple_coin FROM user_explore_maps WHERE user_id = ? AND map_id = ?",
        ).bind(user_id).bind(map_id).fetch_one(&mut *tx).await?);
    }
    let tasks = if gold_delta > 0 || purple_delta > 0 {
        database::db::game::tasks::sync_explore_tasks_in_transaction(&mut tx, user_id).await?
    } else {
        Vec::new()
    };
    explore_runtime::save_map(&mut tx, user_id, &mut stored).await?;
    let mut steps = vec![explore_engine::position_step(&stored.state)];
    steps.extend(explore_engine::changed_object_steps(&stored.state, &changed)?);
    if let Some((gold, purple)) = totals {
        for (id, changed, count) in [(1, gold_delta, gold), (2, purple_delta, purple)] {
            if changed > 0 {
                steps.push(ExploreStep { param: Some(serde_json::json!({
                    "stepType": 5, "id": id, "num": count,
                }).to_string()) });
            }
        }
    }
    tx.commit().await?;
    Ok(Movement { steps, tasks })
}

#[cfg(test)]
mod tests {
    use super::*;
    use database::db::game::explore_runtime::MapState;

    #[tokio::test]
    async fn collecting_and_revisiting_a_coin_is_atomic_and_idempotent() {
        crate::init_test_server_config();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
        config::init(dir.to_str().unwrap()).expect("load isolated game configuration");
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user_id = 930001_i64;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind("isolated-coin-test").execute(&pool).await.unwrap();
        let layout = config::explore_layout::layout(101).unwrap().unwrap();
        let mut state = MapState::new(layout);
        state.x = 15;
        state.y = 2;
        let mut tx = pool.begin().await.unwrap();
        explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
        explore_runtime::insert_map(&mut tx, user_id, &state).await.unwrap();
        explore_runtime::set_current_map(&mut tx, user_id, 101).await.unwrap();
        tx.commit().await.unwrap();

        sqlx::query("CREATE TRIGGER isolated_fail_explore_task BEFORE INSERT ON user_tasks \
            BEGIN SELECT RAISE(ABORT, 'isolated test failure'); END")
            .execute(&pool).await.unwrap();
        assert!(move_player(&pool, user_id, 16, 2).await.is_err());
        let mut tx = pool.begin().await.unwrap();
        let unchanged = explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap();
        assert_eq!((unchanged.state.x, unchanged.state.y), (15, 2));
        assert_eq!(unchanged.state.objects[&362].status & 1, 1);
        tx.commit().await.unwrap();
        sqlx::query("DROP TRIGGER isolated_fail_explore_task").execute(&pool).await.unwrap();

        let movement = move_player(&pool, user_id, 16, 2).await.unwrap();
        let first_task = movement.tasks.iter().find(|task| task.task_id == 210151).unwrap();
        assert_eq!(first_task.progress, 1);
        assert!(first_task.has_finished);
        let steps = movement.steps;
        assert_eq!(steps.len(), 3);
        let coin_step: serde_json::Value = serde_json::from_str(steps[2].param.as_ref().unwrap()).unwrap();
        assert_eq!((coin_step["id"].as_i64(), coin_step["num"].as_i64()), (Some(2), Some(1)));
        move_player(&pool, user_id, 15, 2).await.unwrap();
        assert_eq!(move_player(&pool, user_id, 16, 2).await.unwrap().steps.len(), 1);
        let (gold, purple): (i32, i32) = sqlx::query_as("SELECT gold_coin, purple_coin FROM user_explore_maps \
            WHERE user_id = ? AND map_id = 101").bind(user_id).fetch_one(&pool).await.unwrap();
        assert_eq!((gold, purple), (0, 1));
        let mut tx = pool.begin().await.unwrap();
        let state = explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap();
        assert_eq!(state.state.objects[&362].status & 1, 0);
        assert_eq!((state.state.x, state.state.y), (16, 2));
        tx.commit().await.unwrap();

        let mut manager = crate::task::TaskManager::new(user_id);
        let tasks = manager.get_info(&pool, vec![21]).await.unwrap();
        assert_eq!(tasks.task_info.len(), 57);
        let claim = manager.finish(&pool, 210151).await.unwrap();
        assert_eq!(claim.reply.finish_count, Some(1));
        assert!(manager.finish(&pool, 210151).await.is_err());
        let currency: i32 = sqlx::query_scalar("SELECT quantity FROM currencies WHERE user_id = ? AND currency_id = 5")
            .bind(user_id).fetch_one(&pool).await.unwrap();
        assert_eq!(currency, 8000);
        let tasks = manager.get_info(&pool, vec![21]).await.unwrap();
        let task = tasks.task_info.iter().find(|task| task.id == 210151).unwrap();
        assert_eq!(task.finish_count, Some(1));
        assert!(!task.has_finished);
    }

    #[tokio::test]
    async fn client_ice_slide_collects_crossed_coins_and_restores_its_endpoint() {
        crate::init_test_server_config();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
        config::init(dir.to_str().unwrap()).unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user_id = 930002_i64;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind("isolated-ice-test").execute(&pool).await.unwrap();
        let layout = config::explore_layout::layout(101).unwrap().unwrap();
        let mut state = MapState::new(layout);
        state.x = 20;
        state.y = 9;
        let mut tx = pool.begin().await.unwrap();
        explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
        explore_runtime::insert_map(&mut tx, user_id, &state).await.unwrap();
        explore_runtime::set_current_map(&mut tx, user_id, 101).await.unwrap();
        tx.commit().await.unwrap();

        // The client cannot stop in the middle of this unobstructed slide.
        assert!(move_player(&pool, user_id, 20, 12).await.is_err());
        let mut tx = pool.begin().await.unwrap();
        let saved = explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap();
        assert_eq!((saved.state.x, saved.state.y), (20, 9));
        assert_eq!(saved.state.objects[&367].status & 1, 1);
        assert_eq!(saved.state.objects[&368].status & 1, 1);
        tx.commit().await.unwrap();

        let result = move_player(&pool, user_id, 20, 15).await.unwrap();
        assert_eq!(result.steps.len(), 4);
        let purple: i32 = sqlx::query_scalar("SELECT purple_coin FROM user_explore_maps WHERE user_id = ? AND map_id = 101")
            .bind(user_id).fetch_one(&pool).await.unwrap();
        assert_eq!(purple, 2);
        let mut tx = pool.begin().await.unwrap();
        let saved = explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap();
        assert_eq!((saved.state.x, saved.state.y), (20, 15));
        assert_eq!(saved.state.objects[&367].status & 1, 0);
        assert_eq!(saved.state.objects[&368].status & 1, 0);
        assert!(saved.state.visited_nodes.contains(&(20, 11)) && saved.state.visited_nodes.contains(&(20, 13)));
        tx.commit().await.unwrap();
        assert_eq!(move_player(&pool, user_id, 20, 9).await.unwrap().steps.len(), 1);
    }

    #[test]
    fn ice_slide_stops_at_a_real_obstacle_without_accepting_a_shortcut() {
        let layout = config::explore_layout::layout(101).unwrap().unwrap();
        let mut state = MapState::new(layout);
        state.x = 23;
        state.y = 18;
        let original = state.clone();
        assert!(explore_engine::move_client(&mut state, layout, 19, 18).is_err());
        assert_eq!(state, original);
        assert!(explore_engine::move_client(&mut state, layout, 18, 19).is_err());
        assert_eq!(state, original);
        explore_engine::move_client(&mut state, layout, 18, 18).unwrap();
        assert_eq!((state.x, state.y), (18, 18));
    }

    #[tokio::test]
    async fn elevator_height_survives_reload_and_allows_both_directions() {
        let layout = config::explore_layout::layout(101).unwrap().unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user_id = 930003_i64;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind("isolated-elevator-test").execute(&pool).await.unwrap();
        let mut state = MapState::new(layout);
        state.x = 26;
        state.y = 2;
        let mut tx = pool.begin().await.unwrap();
        explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
        explore_runtime::insert_map(&mut tx, user_id, &state).await.unwrap();
        explore_runtime::set_current_map(&mut tx, user_id, 101).await.unwrap();
        tx.commit().await.unwrap();
        assert!(move_player(&pool, user_id, 28, 2).await.is_err());
        assert_eq!(move_player(&pool, user_id, 27, 2).await.unwrap().steps.len(), 2);
        let mut tx = pool.begin().await.unwrap();
        let saved = explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap();
        let status: serde_json::Value = serde_json::from_str(&saved.state.objects[&507].status2).unwrap();
        assert_eq!(status["height"], 3);
        tx.commit().await.unwrap();
        assert!(move_player(&pool, user_id, 26, 2).await.is_err());
        move_player(&pool, user_id, 28, 2).await.unwrap();
        move_player(&pool, user_id, 27, 2).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let saved = explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap();
        let status: serde_json::Value = serde_json::from_str(&saved.state.objects[&507].status2).unwrap();
        assert_eq!(status["height"], 0);
        tx.commit().await.unwrap();
        move_player(&pool, user_id, 26, 2).await.unwrap();
    }
}
