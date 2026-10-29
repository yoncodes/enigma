use super::{NewTask, TaskType, UserTask, ensure_task, max_finish_count};
use common::time::ServerTime;
use sqlx::{Sqlite, Transaction};

pub async fn sync_score(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    activity_id: i32,
    boss_id: i32,
    total: i32,
    highest: i32,
) -> sqlx::Result<Vec<UserTask>> {
    let listener_param = format!("{boss_id}#{activity_id}");
    let configs = config::configs::get()
        .activity128_task
        .iter()
        .filter(|task| task.is_online != 0 && task.listener_param == listener_param)
        .collect::<Vec<_>>();
    let type_id = TaskType::Activity128.id();
    for config in &configs {
        ensure_task(
            tx,
            NewTask::new(user_id, type_id, config.id, config.activity_id, 0),
        )
        .await?;
    }

    let mut updated = Vec::new();
    for config in configs {
        let progress = match config.listener_type.as_str() {
            "Act128BossHighestPoint" | "Act128BossSpHighestPoint" => highest,
            "Act128BossLayer4TotalPoint" => total,
            listener => {
                return Err(sqlx::Error::Protocol(format!(
                    "unsupported Activity 128 score listener {listener}"
                )));
            }
        };
        if let Some(task) = sync_task(tx, user_id, config.id, progress, config.max_progress).await?
        {
            updated.push(task);
        }
    }
    Ok(updated)
}

async fn sync_task(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    task_id: i32,
    progress: i32,
    max_progress: i32,
) -> sqlx::Result<Option<UserTask>> {
    let type_id = TaskType::Activity128.id();
    let Some(task) = sqlx::query_as::<_, UserTask>(
        "SELECT user_id, type_id, task_id, progress, has_finished, finish_count,
                expiry_time, min_type_id, activity_id, created_at, updated_at
         FROM user_tasks
         WHERE user_id = ? AND type_id = ? AND task_id = ?",
    )
    .bind(user_id)
    .bind(type_id)
    .bind(task_id)
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Ok(None);
    };

    let max_progress = max_progress.max(1);
    let progress = progress.clamp(0, max_progress);
    let has_finished =
        progress >= max_progress && task.finish_count < max_finish_count(type_id, task_id);
    if progress == task.progress && has_finished == task.has_finished {
        return Ok(None);
    }

    let now = ServerTime::now_ms();
    sqlx::query(
        "UPDATE user_tasks SET progress = ?, has_finished = ?, updated_at = ?
         WHERE user_id = ? AND type_id = ? AND task_id = ?",
    )
    .bind(progress)
    .bind(has_finished)
    .bind(now)
    .bind(user_id)
    .bind(type_id)
    .bind(task_id)
    .execute(&mut **tx)
    .await?;

    Ok(Some(UserTask {
        progress,
        has_finished,
        updated_at: now,
        ..task
    }))
}

#[cfg(test)]
mod tests;
