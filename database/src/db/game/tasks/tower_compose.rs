//! Tides tasks are advanced only by committed results from the matching theme.

use anyhow::{Context, Result};
use common::time::ServerTime;
use sonettobuf::TowerComposePlane;
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::models::game::tasks::UserTask;

pub const TOWER_COMPOSE_TASK_TYPE: i32 = 72;

#[derive(Clone, Debug)]
pub struct TowerComposeTaskOutcome {
    pub theme_id: i32,
    pub layer_id: i32,
    pub plane_id: i32,
    pub won: bool,
    pub completed: bool,
    pub boss_level: i32,
    pub score: i32,
    pub planes: Vec<TowerComposePlane>,
}

pub fn compose_task_available(task: &config::tower_compose_task::TowerComposeTask, now: i32) -> bool {
    if task.is_online != 1 { return false; }
    let after_start = task.start_time.trim().is_empty()
        || ServerTime::config_datetime_sec(&task.start_time).is_some_and(|start| now >= start);
    let before_end = task.end_time.trim().is_empty()
        || ServerTime::config_datetime_sec(&task.end_time).is_some_and(|end| now <= end);
    after_start && before_end
}

pub async fn sync_tower_compose_tasks(pool: &SqlitePool, user_id: i64) -> Result<Vec<UserTask>> {
    let mut tx = pool.begin().await?;
    let changed = sync_tower_compose_tasks_in_transaction(&mut tx, user_id, None).await?;
    tx.commit().await?;
    Ok(changed)
}

pub async fn sync_tower_compose_tasks_in_transaction(
    tx: &mut Transaction<'_, Sqlite>, user_id: i64, outcome: Option<&TowerComposeTaskOutcome>,
) -> Result<Vec<UserTask>> {
    let now_ms = ServerTime::now_ms();
    let now = ServerTime::now_sec_i32();
    let tables = config::configs::get();
    let mut updated = Vec::new();
    for task in tables.tower_compose_task.iter().filter(|task| compose_task_available(task, now)) {
        let theme = task.listener_param.split('#').next().and_then(|part| part.parse::<i32>().ok());
        let Some(theme_id) = theme else { continue; };
        let state = super::super::tower_compose::get_theme_state_in_transaction(tx, user_id, theme_id).await?;
        let value = task_progress(task, state.as_ref(), outcome.filter(|result| result.theme_id == theme_id))?;
        let expiry = if task.end_time.trim().is_empty() { 0 } else {
            ServerTime::config_datetime_sec(&task.end_time).context("invalid compose task end time")?
        };
        // The client requests type 72 with minTypeId/activityId 0 and groups by local config.
        sqlx::query("INSERT INTO user_tasks (user_id, type_id, task_id, expiry_time, min_type_id, activity_id, created_at, updated_at) VALUES (?, ?, ?, ?, 0, 0, ?, ?) ON CONFLICT(user_id, type_id, task_id) DO NOTHING")
            .bind(user_id).bind(TOWER_COMPOSE_TASK_TYPE).bind(task.id).bind(expiry).bind(now_ms).bind(now_ms)
            .execute(&mut **tx).await?;
        let mut existing = sqlx::query_as::<_, UserTask>(
            "SELECT user_id, type_id, task_id, progress, has_finished, finish_count, expiry_time, min_type_id, activity_id, created_at, updated_at FROM user_tasks WHERE user_id = ? AND type_id = ? AND task_id = ?",
        ).bind(user_id).bind(TOWER_COMPOSE_TASK_TYPE).bind(task.id).fetch_one(&mut **tx).await?;
        let max = task.max_progress.max(1);
        let progress = existing.progress.max(value.clamp(0, max));
        let finished = progress >= max && existing.finish_count == 0;
        if progress != existing.progress || finished != existing.has_finished {
            sqlx::query("UPDATE user_tasks SET progress = ?, has_finished = ?, updated_at = ? WHERE user_id = ? AND type_id = ? AND task_id = ?")
                .bind(progress).bind(finished).bind(now_ms).bind(user_id).bind(TOWER_COMPOSE_TASK_TYPE).bind(task.id)
                .execute(&mut **tx).await?;
            existing.progress = progress;
            existing.has_finished = finished;
            existing.updated_at = now_ms;
            updated.push(existing);
        }
    }
    Ok(updated)
}

fn task_progress(
    task: &config::tower_compose_task::TowerComposeTask,
    state: Option<&super::super::tower_compose::TowerComposeThemeState>,
    outcome: Option<&TowerComposeTaskOutcome>,
) -> Result<i32> {
    let parts = task.listener_param.split('#').collect::<Vec<_>>();
    let second = || parts.get(1).and_then(|part| part.parse::<i32>().ok());
    let success = outcome.filter(|result| result.won && result.completed);
    let has_mods = |plane: &TowerComposePlane, raw: &str| {
        let selected = plane.mods.iter().flat_map(|group| &group.mods)
            .filter_map(|item| item.mod_id).collect::<std::collections::HashSet<_>>();
        raw.split(',').filter_map(|id| id.parse::<i32>().ok()).all(|id| selected.contains(&id))
    };
    let value = match task.listener_type.as_str() {
        "TowerComposePassLayer" => i32::from(state.is_some_and(|s| s.pass_max_layer_id >= second().unwrap_or(i32::MAX))),
        "TowerComposeBossHighLevel" => i32::from(success.is_some_and(|r| r.boss_level >= second().unwrap_or(i32::MAX))),
        "TowerComposeBossHighScore" => success.map_or(0, |r| r.score),
        "TowerComposeUseMods" => {
            let score = second().unwrap_or(i32::MAX);
            let required = parts.get(2).copied().unwrap_or("");
            i32::from(!required.is_empty() && success.is_some_and(|r| r.score >= score
                && r.planes.iter().any(|plane| plane.result == Some(1) && has_mods(plane, required))))
        }
        "TowerComposeUseModsSettle" => {
            let required = parts.get(1).copied().unwrap_or("");
            i32::from(!required.is_empty() && outcome.is_some_and(|r| r.planes.iter()
                .any(|plane| plane.plane_id == Some(r.plane_id) && has_mods(plane, required))))
        }
        "TowerComposeUseSupportRare" => {
            let rare = second().unwrap_or(-1);
            i32::from(success.is_some_and(|r| r.planes.iter().any(|plane| {
                let support_id = plane.team.as_ref().and_then(|team| team.support_id).unwrap_or_default();
                config::configs::get().tower_compose_support.get(support_id)
                    .and_then(|support| config::configs::get().character.get(support.hero_id))
                    .is_some_and(|hero| hero.rare == rare)
            })))
        }
        "TowerComposeUseSupport" => {
            let required_score = second().unwrap_or(i32::MAX);
            let hero_id = parts.get(2).and_then(|part| part.parse::<i32>().ok()).unwrap_or_default();
            i32::from(success.is_some_and(|r| r.score >= required_score && r.planes.iter().any(|plane| {
                let id = plane.team.as_ref().and_then(|team| team.support_id).unwrap_or_default();
                config::configs::get().tower_compose_support.get(id).is_some_and(|support| support.hero_id == hero_id)
            })))
        }
        _ => 0,
    };
    Ok(value)
}
