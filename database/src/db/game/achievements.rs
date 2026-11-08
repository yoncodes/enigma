use crate::models::game::achievements::Achievement;
use anyhow::Result;
use common::time::ServerTime;
use sqlx::{QueryBuilder, Sqlite, SqlitePool, Transaction};

use super::tasks::TaskEvent;

pub async fn get_achievements(pool: &SqlitePool, user_id: i64) -> Result<Vec<Achievement>> {
    let achievements = sqlx::query_as::<_, Achievement>(
        "SELECT * FROM user_achievements WHERE user_id = ? ORDER BY achievement_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(achievements)
}

pub async fn reconcile_snapshot(pool: &SqlitePool, user_id: i64) -> Result<()> {
    let (player_level, total_login_days): (i32, i32) = sqlx::query_as(
        "SELECT users.level, player_info.total_login_days
         FROM users
         JOIN player_info ON player_info.player_id = users.id
         WHERE users.id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .unwrap_or_default();
    let hero_count: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM heroes WHERE user_id = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    let mut tx = pool.begin().await?;

    for task in config::configs::get().achievement_task.iter() {
        let value = match task.listener_type.as_str() {
            "PlayerLv" => player_level,
            "HeroCount" => hero_count,
            "TotalLoginDays" => total_login_days,
            _ => 0,
        };
        set_snapshot_progress(&mut tx, user_id, task.id, value, task.max_progress).await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn sync_event(
    pool: &SqlitePool,
    user_id: i64,
    event: TaskEvent,
) -> Result<Vec<Achievement>> {
    let mut tx = pool.begin().await?;
    let updated = sync_event_in_transaction(&mut tx, user_id, event).await?;
    tx.commit().await?;
    Ok(updated)
}

pub async fn sync_event_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    event: TaskEvent,
) -> Result<Vec<Achievement>> {
    let now = ServerTime::now_sec_i32();
    let mut updated = Vec::new();

    for task in config::configs::get().achievement_task.iter() {
        let Some(increment) =
            event.achievement_increment(&task.listener_type, &task.listener_param)
        else {
            continue;
        };

        if let Some(achievement) = sqlx::query_as::<_, Achievement>(
            r#"
            INSERT INTO user_achievements (
                user_id, achievement_id, progress, has_finish, is_new,
                finish_time, created_at, updated_at
            ) VALUES (
                ?, ?, MIN(?, ?), ? >= ?, ? >= ?,
                CASE WHEN ? >= ? THEN ? ELSE 0 END, ?, ?
            )
            ON CONFLICT(user_id, achievement_id) DO UPDATE SET
                progress = MIN(?, user_achievements.progress + ?),
                has_finish = user_achievements.progress + ? >= ?,
                is_new = CASE
                    WHEN user_achievements.progress + ? >= ? THEN 1
                    ELSE user_achievements.is_new
                END,
                finish_time = CASE
                    WHEN user_achievements.progress + ? >= ? THEN ?
                    ELSE user_achievements.finish_time
                END,
                updated_at = ?
            WHERE user_achievements.has_finish = 0
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(task.id)
        .bind(increment)
        .bind(task.max_progress)
        .bind(increment)
        .bind(task.max_progress)
        .bind(increment)
        .bind(task.max_progress)
        .bind(increment)
        .bind(task.max_progress)
        .bind(now)
        .bind(now)
        .bind(now)
        .bind(task.max_progress)
        .bind(increment)
        .bind(increment)
        .bind(task.max_progress)
        .bind(increment)
        .bind(task.max_progress)
        .bind(increment)
        .bind(task.max_progress)
        .bind(now)
        .bind(now)
        .fetch_optional(&mut **tx)
        .await?
        {
            updated.push(achievement);
        }
    }

    Ok(updated)
}

async fn set_snapshot_progress(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    achievement_id: i32,
    value: i32,
    max_progress: i32,
) -> Result<()> {
    let now = i64::from(ServerTime::now_sec_i32());
    let progress = value.min(max_progress);
    let finished = value >= max_progress;

    sqlx::query(
        r#"
        INSERT INTO user_achievements (
            user_id, achievement_id, progress, has_finish, is_new, finish_time, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(user_id, achievement_id) DO UPDATE SET
            progress = MAX(user_achievements.progress, excluded.progress),
            has_finish = MAX(user_achievements.has_finish, excluded.has_finish),
            is_new = CASE
                WHEN user_achievements.has_finish = 0 AND excluded.has_finish = 1 THEN 1
                ELSE user_achievements.is_new
            END,
            finish_time = CASE
                WHEN user_achievements.has_finish = 0 AND excluded.has_finish = 1
                    THEN excluded.finish_time
                ELSE user_achievements.finish_time
            END,
            updated_at = excluded.updated_at
        WHERE excluded.progress > user_achievements.progress
           OR (user_achievements.has_finish = 0 AND excluded.has_finish = 1)
        "#,
    )
    .bind(user_id)
    .bind(achievement_id)
    .bind(progress)
    .bind(finished)
    .bind(finished)
    .bind(if finished { now as i32 } else { 0 })
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn update_achievement_progress(
    pool: &SqlitePool,
    user_id: i64,
    achievement_id: i32,
    progress: i32,
) -> Result<()> {
    let now = i64::from(ServerTime::now_sec_i32());

    sqlx::query(
        r#"
        INSERT INTO user_achievements (
            user_id, achievement_id, progress, has_finish, is_new, finish_time, created_at, updated_at
        ) VALUES (?, ?, ?, 0, 1, 0, ?, ?)
        ON CONFLICT(user_id, achievement_id) DO UPDATE SET
            progress = excluded.progress,
            is_new = 1,
            updated_at = excluded.updated_at
        "#
    )
    .bind(user_id)
    .bind(achievement_id)
    .bind(progress)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn finish_achievement(
    pool: &SqlitePool,
    user_id: i64,
    achievement_id: i32,
) -> Result<()> {
    let now = i64::from(ServerTime::now_sec_i32());
    let finish_time = now as i32;

    sqlx::query(
        r#"
        UPDATE user_achievements
        SET has_finish = 1, finish_time = ?, is_new = 1, updated_at = ?
        WHERE user_id = ? AND achievement_id = ?
        "#,
    )
    .bind(finish_time)
    .bind(now)
    .bind(user_id)
    .bind(achievement_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn clear_new_flags(
    pool: &SqlitePool,
    user_id: i64,
    achievement_ids: Vec<i32>,
) -> Result<Vec<Achievement>> {
    if achievement_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut query =
        QueryBuilder::<Sqlite>::new("UPDATE user_achievements SET is_new = 0, updated_at = ");
    query
        .push_bind(i64::from(ServerTime::now_sec_i32()))
        .push(" WHERE user_id = ")
        .push_bind(user_id)
        .push(" AND achievement_id IN (");
    let mut ids = query.separated(", ");
    for achievement_id in achievement_ids {
        ids.push_bind(achievement_id);
    }
    ids.push_unseparated(") RETURNING *");

    Ok(query.build_query_as().fetch_all(pool).await?)
}
