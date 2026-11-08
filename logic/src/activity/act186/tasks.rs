use super::*;
use common::time::ServerTime;
use std::collections::{BTreeMap, HashMap};

pub struct Act186TaskClaim {
    pub reply: FinishAllAct186TaskReply,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
    pub like_infos: Vec<Act186LikeInfo>,
}

pub async fn sync(
    db: &SqlitePool,
    player_id: i64,
    event: crate::task::TaskEvent,
) -> Result<Vec<Act186TaskPush>, AppError> {
    sync_at(db, player_id, event, ServerTime::now_ms()).await
}

pub(super) async fn sync_at(
    db: &SqlitePool,
    player_id: i64,
    event: crate::task::TaskEvent,
    now: i64,
) -> Result<Vec<Act186TaskPush>, AppError> {
    let tables = config::configs::get();
    let stages = active_stages(tables, now);
    let mut tx = db.begin().await?;
    let mut updates = BTreeMap::<i32, Vec<Act186TaskInfo>>::new();

    for row in tables
        .actvity186_task
        .iter()
        .filter(|row| row.is_online != 0 && task_is_active(row, &stages))
    {
        let Some(increment) = event.progress(&row.listener_type, &row.listener_param) else {
            continue;
        };
        let progress = sqlx::query_scalar::<_, i32>(
            "INSERT INTO user_activity_state
                (user_id, activity_id, kind, entry_id, state, progress, ext, updated_at)
             VALUES (?, ?, ?, ?, 0, MIN(?, ?), '', ?)
             ON CONFLICT(user_id, activity_id, kind, entry_id) DO UPDATE SET
                progress = MIN(?, user_activity_state.progress + excluded.progress),
                updated_at = excluded.updated_at
             WHERE user_activity_state.state = 0
               AND user_activity_state.progress < ?
             RETURNING progress",
        )
        .bind(player_id)
        .bind(row.activity_id)
        .bind(ActivityStateKind::Act186Task.id())
        .bind(row.id)
        .bind(increment)
        .bind(row.max_progress)
        .bind(now)
        .bind(row.max_progress)
        .bind(row.max_progress)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(progress) = progress else {
            continue;
        };
        let activity_end = tables
            .actvity186_stage
            .iter()
            .filter(|stage| stage.activity_id == row.activity_id)
            .map(|stage| parse_time_millis(&stage.end_time))
            .max()
            .unwrap_or_default();
        updates
            .entry(row.activity_id)
            .or_default()
            .push(Act186TaskInfo {
                task_id: Some(row.id),
                progress: Some(progress),
                expire_time: Some(task_expire_time(row.loop_type, activity_end, now)),
                has_get_bonus: Some(false),
            });
    }
    tx.commit().await?;

    Ok(updates
        .into_iter()
        .map(|(activity_id, mut act186_tasks)| {
            act186_tasks.sort_by_key(|task| task.task_id.unwrap_or_default());
            Act186TaskPush {
                activity_id: Some(activity_id),
                act186_tasks,
                delete_tasks: Vec::new(),
            }
        })
        .collect())
}

pub async fn sync_login(
    db: &SqlitePool,
    player_id: i64,
    new_day: bool,
    new_week: bool,
    now: i64,
) -> Result<(), AppError> {
    use database::db::game::tasks::TaskLoopType;

    let tables = config::configs::get();
    let stages = active_stages(tables, now);
    let rows = tables
        .actvity186_task
        .iter()
        .filter(|row| row.is_online != 0 && task_is_active(row, &stages))
        .collect::<Vec<_>>();
    let mut tx = db.begin().await?;

    for row in &rows {
        let reset = match TaskLoopType::from_id(row.loop_type) {
            Some(TaskLoopType::Daily) => new_day,
            Some(TaskLoopType::Weekly) => new_week,
            _ => false,
        };
        if reset {
            sqlx::query(
                "UPDATE user_activity_state
                 SET state = 0, progress = 0, updated_at = ?
                 WHERE user_id = ? AND activity_id = ? AND kind = ? AND entry_id = ?",
            )
            .bind(now)
            .bind(player_id)
            .bind(row.activity_id)
            .bind(ActivityStateKind::Act186Task.id())
            .bind(row.id)
            .execute(&mut *tx)
            .await?;
        }
    }

    for row in rows
        .into_iter()
        .filter(|row| row.listener_type == "LoginDays")
    {
        sqlx::query(
            "INSERT INTO user_activity_state
                (user_id, activity_id, kind, entry_id, state, progress, ext, updated_at)
             VALUES (?, ?, ?, ?, 0, 1, '', ?)
             ON CONFLICT(user_id, activity_id, kind, entry_id) DO UPDATE SET
                progress = MIN(?, CASE
                    WHEN ? THEN user_activity_state.progress + 1
                    ELSE MAX(user_activity_state.progress, 1)
                END),
                updated_at = excluded.updated_at
             WHERE user_activity_state.state = 0",
        )
        .bind(player_id)
        .bind(row.activity_id)
        .bind(ActivityStateKind::Act186Task.id())
        .bind(row.id)
        .bind(now)
        .bind(row.max_progress)
        .bind(new_day)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

fn active_stages(tables: &config::GameDB, now: i64) -> HashMap<i32, i32> {
    tables
        .actvity186_stage
        .iter()
        .filter(|row| {
            let start = parse_time_millis(&row.start_time);
            let end = parse_time_millis(&row.end_time);
            start <= now && now <= end
        })
        .map(|row| (row.activity_id, row.stage_id))
        .collect()
}

fn task_is_active(
    row: &config::actvity186_task::Actvity186Task,
    stages: &HashMap<i32, i32>,
) -> bool {
    stages
        .get(&row.activity_id)
        .is_some_and(|stage| row.accept_stage.parse::<i32>() == Ok(*stage))
}

pub async fn finish_all(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
) -> Result<Act186TaskClaim, AppError> {
    let activity_id = activity_id.ok_or(AppError::InvalidRequest)?;
    let tables = config::configs::get();
    let states =
        activity_state::get(db, player_id, activity_id, ActivityStateKind::Act186Task).await?;
    let rows = tables
        .actvity186_task
        .iter()
        .filter(|row| row.activity_id == activity_id && row.is_online != 0)
        .filter(|row| {
            states
                .get(&row.id)
                .is_some_and(|(state, progress, _)| *state == 0 && *progress >= row.max_progress)
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return Err(AppError::InvalidRequest);
    }

    let mut tx = db.begin().await?;
    let mut task_ids = Vec::new();
    let mut rewards = reward::RewardSet::default();
    let mut likes = BTreeMap::<i32, i32>::new();
    for row in rows {
        let updated = sqlx::query(
            "UPDATE user_activity_state
             SET state = 1, updated_at = ?
             WHERE user_id = ? AND activity_id = ? AND kind = ? AND entry_id = ?
               AND state = 0 AND progress >= ?",
        )
        .bind(ServerTime::now_ms())
        .bind(player_id)
        .bind(activity_id)
        .bind(ActivityStateKind::Act186Task.id())
        .bind(row.id)
        .bind(row.max_progress)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            continue;
        }

        let (task_rewards, task_likes) = task_bonus(&row.bonus)?;
        for (like_type, value) in task_likes {
            if !tables
                .actvity186_like
                .iter()
                .any(|like| like.activity_id == activity_id && like.r#type == like_type)
            {
                return Err(AppError::InvalidRequest);
            }
            likes
                .entry(like_type)
                .and_modify(|total| *total = total.saturating_add(value))
                .or_insert(value);
        }
        rewards.extend(task_rewards);
        task_ids.push(row.id);
    }
    if task_ids.is_empty() {
        return Err(AppError::InvalidRequest);
    }

    let mut like_infos = Vec::new();
    for (like_type, value) in likes {
        let total = sqlx::query_scalar::<_, i32>(
            "INSERT INTO user_activity_state
                (user_id, activity_id, kind, entry_id, state, progress, ext, updated_at)
             VALUES (?, ?, ?, ?, ?, 0, '', ?)
             ON CONFLICT(user_id, activity_id, kind, entry_id) DO UPDATE SET
                state = MIN(2147483647, user_activity_state.state + excluded.state),
                updated_at = excluded.updated_at
             RETURNING state",
        )
        .bind(player_id)
        .bind(activity_id)
        .bind(ActivityStateKind::Act186Like.id())
        .bind(like_type)
        .bind(value)
        .bind(ServerTime::now_ms())
        .fetch_one(&mut *tx)
        .await?;
        like_infos.push(Act186LikeInfo {
            like_type: Some(like_type),
            value: Some(total),
        });
    }

    task_ids.sort_unstable();
    let material_changes = rewards.material_changes();
    let applied = reward::RewardManager::new(player_id)
        .apply_in_transaction(&mut tx, db, rewards)
        .await?;
    tx.commit().await?;

    Ok(Act186TaskClaim {
        reply: FinishAllAct186TaskReply {
            activity_id: Some(activity_id),
            task_ids,
        },
        rewards: applied,
        material_changes,
        like_infos,
    })
}

fn task_bonus(value: &str) -> Result<(reward::RewardSet, BTreeMap<i32, i32>), AppError> {
    let mut rewards = reward::RewardSet::default();
    let mut likes = BTreeMap::new();
    for part in value.split('|') {
        let fields = part
            .split('#')
            .map(str::parse::<i32>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::InvalidRequest)?;
        if fields.len() != 3 || fields[1] < 0 || fields[2] < 0 {
            return Err(AppError::InvalidRequest);
        }
        if fields[0] == reward::RewardMaterialType::Act186Like.id() as i32 {
            likes
                .entry(fields[1])
                .and_modify(|total: &mut i32| *total = total.saturating_add(fields[2]))
                .or_insert(fields[2]);
        } else {
            rewards.extend(reward::parse_strict(part)?);
        }
    }
    Ok((rewards, likes))
}
