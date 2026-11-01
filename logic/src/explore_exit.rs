//! Exploration map completion, separate from individual battles.

use anyhow::{Context, Result, ensure};
use database::db::game::{dungeons, explore_runtime, open_infos};
use sonettobuf::{DungeonUpdatePush, ExploreStep, OpenInfo};
use sqlx::SqlitePool;

pub struct ExitResult {
    pub chapter_id: i32,
    pub episode_id: i32,
    pub dungeon_update: DungeonUpdatePush,
    pub open_infos: Vec<OpenInfo>,
    pub steps: Vec<ExploreStep>,
}

pub async fn complete(pool: &SqlitePool, user_id: i64, exit_id: i32) -> Result<ExitResult> {
    let mut tx = pool.begin().await?;
    let map_id = explore_runtime::current_map(&mut tx, user_id).await?.context("no current exploration map")?;
    let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map")?;
    let (area, exit) = layout.areas.iter().find_map(|area| {
        area.units.iter().find(|unit| unit.id == exit_id).map(|unit| (area, unit))
    }).context("unknown exploration exit")?;
    ensure!(exit.kind == 214 && exit.trigger_effects.is_empty() && exit.done_effects.is_empty(),
        "unsupported exploration exit");
    let pending: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_explore_pending_battles WHERE user_id = ?)",
    ).bind(user_id).fetch_one(&mut *tx).await?;
    ensure!(!pending, "exploration battle is pending");
    let mut state = explore_runtime::load_map(&mut tx, user_id, map_id).await?
        .context("exploration map state is missing")?;
    ensure!(area.id == 0 || state.state.areas.contains(&area.id), "exit area is not open");
    let object = state.state.objects.get(&exit_id).context("exit state is missing")?;
    ensure!(object.kind == 214 && object.status & 7 == 7 && object.status & 8 == 0,
        "exploration exit is not active");
    let dx = i64::from(state.state.x) - i64::from(object.x);
    let dy = i64::from(state.state.y) - i64::from(object.y);
    ensure!(dx.abs() + dy.abs() <= 1, "exploration exit is too far away");
    state.state.challenge_count = state.state.challenge_count.checked_add(1).context("challenge count overflow")?;
    let (dungeon, chapter_type_nums) = dungeons::update_dungeon_progress_in_transaction(
        &mut tx, user_id, layout.chapter_id, layout.episode_id, 1,
    ).await?;
    let tables = config::configs::get();
    for next in config::explore_layout::layouts()? {
        let episode = tables.episode.get(next.episode_id).context("exploration episode is missing")?;
        if episode.pre_episode != layout.episode_id { continue; }
        sqlx::query("INSERT INTO user_explore_unlocked_maps (user_id, map_id) VALUES (?, ?) \
            ON CONFLICT(user_id, map_id) DO NOTHING")
            .bind(user_id).bind(next.map_id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO user_explore_chapters (user_id, chapter_id) VALUES (?, ?) \
            ON CONFLICT(user_id, chapter_id) DO NOTHING")
            .bind(user_id).bind(next.chapter_id).execute(&mut *tx).await?;
        crate::explore_entry::ensure_map_summary(&mut tx, user_id, next).await?;
    }
    let mut chapter_complete = true;
    for chapter_map in config::explore_layout::layouts()?.iter().filter(|map| map.chapter_id == layout.chapter_id) {
        let cleared: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_dungeons \
            WHERE user_id = ? AND chapter_id = ? AND episode_id = ? AND star > 0)")
            .bind(user_id).bind(chapter_map.chapter_id).bind(chapter_map.episode_id)
            .fetch_one(&mut *tx).await?;
        chapter_complete &= cleared;
    }
    if chapter_complete {
        sqlx::query("INSERT INTO user_explore_chapters (user_id, chapter_id, is_finish) VALUES (?, ?, 1) \
            ON CONFLICT(user_id, chapter_id) DO UPDATE SET is_finish = 1")
            .bind(user_id).bind(layout.chapter_id).execute(&mut *tx).await?;
    }
    let open_infos = open_infos::reconcile_progression_in_transaction(&mut tx, user_id).await?
        .into_iter().map(Into::into).collect();
    explore_runtime::save_map(&mut tx, user_id, &mut state).await?;
    tx.commit().await?;
    Ok(ExitResult {
        chapter_id: layout.chapter_id,
        episode_id: layout.episode_id,
        dungeon_update: DungeonUpdatePush {
            dungeon_info: Some(dungeon.into()),
            chapter_type_nums: chapter_type_nums.into_iter().map(Into::into).collect(),
        },
        open_infos,
        steps: vec![ExploreStep { param: Some(serde_json::json!({"stepType": 11}).to_string()) }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn exit_requires_activation_and_unlocks_only_configured_successors() {
        crate::init_test_server_config();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
        config::init(dir.to_str().unwrap()).unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        let user_id = 970001_i64;
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(user_id).bind("isolated-exit-test").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO user_open_infos (user_id, open_id, is_open) VALUES (?, 47, 1)")
            .bind(user_id).execute(&pool).await.unwrap();
        crate::explore_entry::change_map(&pool, user_id, 101).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let mut state = explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap();
        state.state.x = 42;
        state.state.y = 2;
        explore_runtime::save_map(&mut tx, user_id, &mut state).await.unwrap();
        tx.commit().await.unwrap();
        assert!(complete(&pool, user_id, 394).await.is_err());
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_dungeons WHERE user_id = ? AND episode_id = 140101")
            .bind(user_id).fetch_one(&pool).await.unwrap();
        assert_eq!(count, 0);
        // Put the test player next to the real archive which activates this exit.
        // This does not attempt to simulate the entire route through the map.
        let mut tx = pool.begin().await.unwrap();
        let mut state = explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap();
        state.state.x = 39;
        state.state.y = 0;
        explore_runtime::save_map(&mut tx, user_id, &mut state).await.unwrap();
        tx.commit().await.unwrap();
        let (archive_id, steps) = crate::explore_engine::collect_basic_archive(&pool, user_id, 518).await.unwrap();
        assert_eq!(archive_id, 140104);
        assert_eq!(steps.len(), 3);
        let mut tx = pool.begin().await.unwrap();
        let mut state = explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap();
        assert_eq!(state.state.objects[&394].status & 4, 4);
        assert_eq!(state.state.objects[&518].status & 4, 4);
        state.state.x = 42;
        state.state.y = 2;
        explore_runtime::save_map(&mut tx, user_id, &mut state).await.unwrap();
        tx.commit().await.unwrap();
        let result = complete(&pool, user_id, 394).await.unwrap();
        assert_eq!((result.chapter_id, result.episode_id), (1401, 140101));
        assert_eq!(result.dungeon_update.dungeon_info.unwrap().star, Some(1));
        let unlocked: Vec<i32> = sqlx::query_scalar("SELECT map_id FROM user_explore_unlocked_maps \
            WHERE user_id = ? ORDER BY map_id")
            .bind(user_id).fetch_all(&pool).await.unwrap();
        assert_eq!(unlocked, vec![101, 102, 201, 301]);
        let completed: bool = sqlx::query_scalar("SELECT is_finish FROM user_explore_chapters WHERE user_id = ? AND chapter_id = 1401")
            .bind(user_id).fetch_one(&pool).await.unwrap();
        assert!(!completed);
        let mut tx = pool.begin().await.unwrap();
        let saved = explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap();
        assert_eq!(saved.state.challenge_count, 1);
        tx.commit().await.unwrap();
    }
}
