use crate::models::game::tasks::UserTask;
use common::time::ServerTime;
use sqlx::{Sqlite, SqlitePool, Transaction};

use super::{TaskType, ensure_tasks_for_type_in_transaction, max_finish_count};

#[cfg(test)]
mod tests;

pub async fn sync_weekwalk_tasks(pool: &SqlitePool, user_id: i64) -> sqlx::Result<Vec<UserTask>> {
    let mut tx = pool.begin().await?;
    let updated = sync_weekwalk_tasks_in_transaction(&mut tx, user_id).await?;
    tx.commit().await?;
    Ok(updated)
}

pub(crate) async fn sync_weekwalk_tasks_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
) -> sqlx::Result<Vec<UserTask>> {
    ensure_tasks_for_type_in_transaction(tx, user_id, TaskType::WeekWalk).await?;
    let battles = sqlx::query_as::<_, (i32, i32, i32)>(
        "SELECT map_id, battle_id, star
         FROM user_weekwalk_battles WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await?;
    let issue_id = current_issue(tx, user_id).await?;
    let tables = config::configs::get();
    let mut updated = Vec::new();

    for task in tables.task_weekwalk.iter() {
        if !period_applies(&task.periods, issue_id).map_err(|reason| {
            sqlx::Error::Protocol(format!(
                "invalid Weekwalk task {} periods {:?}: {reason}",
                task.id, task.periods
            ))
        })? {
            continue;
        }
        let progress = task_progress(&task.listener_type, &task.listener_param, &battles, tables)
            .map_err(|reason| {
            sqlx::Error::Protocol(format!(
                "invalid Weekwalk task {} listener {:?} params {:?}: {reason}",
                task.id, task.listener_type, task.listener_param
            ))
        })?;
        if let Some(task) = sync_progress(tx, user_id, task.id, progress, task.max_progress).await?
        {
            updated.push(task);
        }
    }

    Ok(updated)
}

pub(super) async fn task_applies_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    task_id: i32,
) -> sqlx::Result<bool> {
    let Some(task) = config::configs::get().task_weekwalk.get(task_id) else {
        return Ok(false);
    };
    period_applies(&task.periods, current_issue(tx, user_id).await?).map_err(|reason| {
        sqlx::Error::Protocol(format!(
            "invalid Weekwalk task {} periods {:?}: {reason}",
            task.id, task.periods
        ))
    })
}

async fn current_issue(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
) -> sqlx::Result<Option<i32>> {
    sqlx::query_scalar("SELECT issue_id FROM user_weekwalk_info WHERE user_id = ?")
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await
}

fn period_applies(raw: &str, issue_id: Option<i32>) -> Result<bool, String> {
    if raw.is_empty() {
        return Ok(true);
    }
    let periods = raw
        .split('#')
        .map(str::parse::<i32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let [start, end] = periods.as_slice() else {
        return Err("expected start and end periods".into());
    };
    if start > end {
        return Err("start period exceeds end period".into());
    }
    Ok(issue_id.is_some_and(|issue_id| (*start..=*end).contains(&issue_id)))
}

async fn sync_progress(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    task_id: i32,
    progress: i32,
    max_progress: i32,
) -> sqlx::Result<Option<UserTask>> {
    let Some(task) = sqlx::query_as::<_, UserTask>(
        "SELECT user_id, type_id, task_id, progress, has_finished, finish_count,
                expiry_time, min_type_id, activity_id, created_at, updated_at
         FROM user_tasks
         WHERE user_id = ? AND type_id = ? AND task_id = ?",
    )
    .bind(user_id)
    .bind(TaskType::WeekWalk.id())
    .bind(task_id)
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Ok(None);
    };
    let max_progress = max_progress.max(1);
    let progress = progress.clamp(0, max_progress);
    let has_finished = progress >= max_progress
        && task.finish_count < max_finish_count(TaskType::WeekWalk.id(), task_id);
    if progress == task.progress && has_finished == task.has_finished {
        return Ok(None);
    }
    sqlx::query(
        "UPDATE user_tasks SET progress = ?, has_finished = ?, updated_at = ?
         WHERE user_id = ? AND type_id = ? AND task_id = ?",
    )
    .bind(progress)
    .bind(has_finished)
    .bind(ServerTime::now_ms())
    .bind(user_id)
    .bind(TaskType::WeekWalk.id())
    .bind(task_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query_as::<_, UserTask>(
        "SELECT user_id, type_id, task_id, progress, has_finished, finish_count,
                expiry_time, min_type_id, activity_id, created_at, updated_at
         FROM user_tasks
         WHERE user_id = ? AND type_id = ? AND task_id = ?",
    )
    .bind(user_id)
    .bind(TaskType::WeekWalk.id())
    .bind(task_id)
    .fetch_optional(&mut **tx)
    .await
}

fn task_progress(
    listener: &str,
    raw_params: &str,
    battles: &[(i32, i32, i32)],
    tables: &config::GameDB,
) -> Result<i32, String> {
    let params = raw_params
        .split('#')
        .map(str::parse::<i32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    match listener {
        "WeekwalkStar" => {
            let [layer] = params.as_slice() else {
                return Err("expected one layer parameter".into());
            };
            Ok(stars_matching(battles, tables, |map| map.layer == *layer))
        }
        "WeekwalkStarTotal" => {
            if params.is_empty() {
                return Err("expected at least one map type parameter".into());
            }
            Ok(stars_matching(battles, tables, |map| {
                params.contains(&map.r#type)
            }))
        }
        "WeekwalkBattle" => {
            let [layer, index] = params.as_slice() else {
                return Err("expected layer and fragment parameters".into());
            };
            if !matches!(index, 1 | 2) {
                return Err(format!("invalid fragment index {index}"));
            }
            let Some(map) = tables.weekwalk.iter().find(|map| {
                map.layer == *layer && battles.iter().any(|(map_id, _, _)| *map_id == map.id)
            }) else {
                return Ok(0);
            };
            let battle_id = if *index == 1 {
                map.fight_id_front
            } else {
                map.fight_id_rear
            };
            Ok(i32::from(battles.iter().any(|(map_id, id, star)| {
                *map_id == map.id && *id == battle_id && *star > 0
            })))
        }
        _ => Err(format!("unsupported listener {listener:?}")),
    }
}

fn stars_matching(
    battles: &[(i32, i32, i32)],
    tables: &config::GameDB,
    matches: impl Fn(&config::weekwalk::Weekwalk) -> bool,
) -> i32 {
    battles
        .iter()
        .filter_map(|(map_id, _, star)| {
            tables
                .weekwalk
                .get(*map_id)
                .filter(|map| matches(map))
                .map(|_| (*star).max(0))
        })
        .sum()
}
