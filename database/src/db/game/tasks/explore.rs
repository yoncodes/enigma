//! Exploration collection tasks are derived from persisted map totals.

use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use common::time::ServerTime;
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::models::game::tasks::UserTask;

pub const EXPLORE_TASK_TYPE: i32 = 21;

pub async fn sync_explore_tasks(pool: &SqlitePool, user_id: i64) -> Result<Vec<UserTask>> {
    let mut tx = pool.begin().await?;
    let tasks = sync_explore_tasks_in_transaction(&mut tx, user_id).await?;
    tx.commit().await?;
    Ok(tasks)
}

pub async fn sync_explore_tasks_in_transaction(
    tx: &mut Transaction<'_, Sqlite>, user_id: i64,
) -> Result<Vec<UserTask>> {
    let has_profile: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_explore_runtime_profiles WHERE user_id = ?)",
    ).bind(user_id).fetch_one(&mut **tx).await?;
    if !has_profile { return Ok(Vec::new()); }
    let maps: Vec<(i32, i32, i32)> = sqlx::query_as(
        "SELECT map_id, gold_coin, purple_coin FROM user_explore_maps WHERE user_id = ?",
    ).bind(user_id).fetch_all(&mut **tx).await?;
    let mut totals = BTreeMap::<i32, (i32, i32)>::new();
    for (map_id, gold, purple) in maps {
        ensure!(gold >= 0 && purple >= 0, "invalid exploration coin totals");
        let layout = config::explore_layout::layout(map_id)?.context("unknown exploration map in progress")?;
        let entry = totals.entry(layout.chapter_id).or_default();
        entry.0 = entry.0.checked_add(gold).context("exploration coin total overflow")?;
        entry.1 = entry.1.checked_add(purple).context("exploration coin total overflow")?;
    }
    let mut updated = Vec::new();
    let now = ServerTime::now_ms();
    for task in config::explore_tasks::tasks()?.iter().filter(|task| task.is_online != 0) {
        let (chapter, kind) = task.collection()?;
        let current = totals.get(&chapter).copied().unwrap_or_default();
        let value = match kind { 1 => current.0, 2 => current.1, _ => unreachable!() };
        sqlx::query("INSERT INTO user_tasks (user_id, type_id, task_id, expiry_time, min_type_id, activity_id, created_at, updated_at) \
            VALUES (?, ?, ?, 0, 0, 0, ?, ?) ON CONFLICT(user_id, type_id, task_id) DO NOTHING")
            .bind(user_id).bind(EXPLORE_TASK_TYPE).bind(task.id).bind(now).bind(now)
            .execute(&mut **tx).await?;
        let mut existing = sqlx::query_as::<_, UserTask>(
            "SELECT user_id, type_id, task_id, progress, has_finished, finish_count, expiry_time, min_type_id, \
             activity_id, created_at, updated_at FROM user_tasks WHERE user_id = ? AND type_id = ? AND task_id = ?",
        ).bind(user_id).bind(EXPLORE_TASK_TYPE).bind(task.id).fetch_one(&mut **tx).await?;
        ensure!(existing.progress <= task.max_progress, "existing exploration task progress is incompatible");
        let progress = existing.progress.max(value.min(task.max_progress));
        let finished = progress >= task.max_progress && existing.finish_count == 0;
        if progress != existing.progress || finished != existing.has_finished {
            sqlx::query("UPDATE user_tasks SET progress = ?, has_finished = ?, updated_at = ? \
                WHERE user_id = ? AND type_id = ? AND task_id = ?")
                .bind(progress).bind(finished).bind(now).bind(user_id).bind(EXPLORE_TASK_TYPE).bind(task.id)
                .execute(&mut **tx).await?;
            existing.progress = progress;
            existing.has_finished = finished;
            existing.updated_at = now;
            updated.push(existing);
        }
    }
    Ok(updated)
}
