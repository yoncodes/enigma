use crate::models::game::tasks::UserTaskActivity;
use common::time::ServerTime;
use sqlx::SqlitePool;

pub async fn list_activity(
    pool: &SqlitePool,
    user_id: i64,
    type_ids: Vec<i32>,
) -> sqlx::Result<Vec<UserTaskActivity>> {
    if type_ids.is_empty() {
        return sqlx::query_as::<_, UserTaskActivity>(
            "SELECT user_id, type_id, define_id, value, gain_value,
                    expiry_time, created_at, updated_at
             FROM user_task_activity
             WHERE user_id = ?
             ORDER BY type_id",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await;
    }

    let placeholders = std::iter::repeat_n("?", type_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT user_id, type_id, define_id, value, gain_value,
                expiry_time, created_at, updated_at
         FROM user_task_activity
         WHERE user_id = ? AND type_id IN ({})
         ORDER BY type_id",
        placeholders
    );
    let mut query = sqlx::query_as::<_, UserTaskActivity>(&sql).bind(user_id);
    for type_id in type_ids {
        query = query.bind(type_id);
    }
    query.fetch_all(pool).await
}

pub async fn add_activity(
    pool: &SqlitePool,
    user_id: i64,
    type_id: i32,
    value: i32,
    expiry_time: i32,
) -> sqlx::Result<Option<UserTaskActivity>> {
    if value <= 0 {
        return Ok(None);
    }

    let now = ServerTime::now_ms();
    sqlx::query(
        "INSERT INTO user_task_activity
         (user_id, type_id, value, expiry_time, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(user_id, type_id) DO UPDATE SET
            value = user_task_activity.value + excluded.value,
            expiry_time = CASE
                WHEN excluded.expiry_time != 0 THEN excluded.expiry_time
                ELSE user_task_activity.expiry_time
            END,
            updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(type_id)
    .bind(value)
    .bind(expiry_time)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    get_activity(pool, user_id, type_id).await
}

pub(super) async fn reset_activity(
    pool: &SqlitePool,
    user_id: i64,
    type_id: i32,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE user_task_activity
         SET define_id = 0, value = 0, gain_value = 0, updated_at = ?
         WHERE user_id = ? AND type_id = ?",
    )
    .bind(ServerTime::now_ms())
    .bind(user_id)
    .bind(type_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn add_activity_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
    type_id: i32,
    value: i32,
    expiry_time: i32,
) -> sqlx::Result<Option<UserTaskActivity>> {
    if value <= 0 {
        return Ok(None);
    }
    let now = ServerTime::now_ms();
    sqlx::query(
        "INSERT INTO user_task_activity
         (user_id, type_id, value, expiry_time, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(user_id, type_id) DO UPDATE SET
            value = user_task_activity.value + excluded.value,
            expiry_time = CASE WHEN excluded.expiry_time != 0
                THEN excluded.expiry_time ELSE user_task_activity.expiry_time END,
            updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(type_id)
    .bind(value)
    .bind(expiry_time)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    sqlx::query_as::<_, UserTaskActivity>(
        "SELECT user_id, type_id, define_id, value, gain_value,
                expiry_time, created_at, updated_at
         FROM user_task_activity WHERE user_id = ? AND type_id = ?",
    )
    .bind(user_id)
    .bind(type_id)
    .fetch_optional(&mut **tx)
    .await
}

pub async fn claim_activity_bonus(
    pool: &SqlitePool,
    user_id: i64,
    type_id: i32,
    define_id: i32,
    need_activity: i32,
) -> sqlx::Result<(UserTaskActivity, bool)> {
    let Some(activity) = get_activity(pool, user_id, type_id).await? else {
        return Ok((empty_activity(user_id, type_id), false));
    };

    let claimable = define_id == activity.define_id + 1
        && activity.value - activity.gain_value >= need_activity;
    if !claimable {
        return Ok((activity, false));
    }

    let now = ServerTime::now_ms();
    sqlx::query(
        "UPDATE user_task_activity
         SET define_id = ?, gain_value = gain_value + ?, updated_at = ?
         WHERE user_id = ? AND type_id = ?",
    )
    .bind(define_id)
    .bind(need_activity)
    .bind(now)
    .bind(user_id)
    .bind(type_id)
    .execute(pool)
    .await?;

    Ok((
        get_activity(pool, user_id, type_id)
            .await?
            .unwrap_or_else(|| empty_activity(user_id, type_id)),
        true,
    ))
}

pub async fn claim_activity_bonus_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
    type_id: i32,
    define_id: i32,
    need_activity: i32,
) -> sqlx::Result<(UserTaskActivity, bool)> {
    let Some(activity) = sqlx::query_as::<_, UserTaskActivity>(
        "SELECT user_id, type_id, define_id, value, gain_value,
                expiry_time, created_at, updated_at
         FROM user_task_activity WHERE user_id = ? AND type_id = ?",
    )
    .bind(user_id)
    .bind(type_id)
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Ok((empty_activity(user_id, type_id), false));
    };
    if define_id != activity.define_id + 1 || activity.value - activity.gain_value < need_activity {
        return Ok((activity, false));
    }
    let result = sqlx::query(
        "UPDATE user_task_activity
         SET define_id = ?, gain_value = gain_value + ?, updated_at = ?
         WHERE user_id = ? AND type_id = ? AND define_id = ? AND gain_value = ?",
    )
    .bind(define_id)
    .bind(need_activity)
    .bind(ServerTime::now_ms())
    .bind(user_id)
    .bind(type_id)
    .bind(activity.define_id)
    .bind(activity.gain_value)
    .execute(&mut **tx)
    .await?;
    if result.rows_affected() != 1 {
        return Ok((activity, false));
    }
    let updated = sqlx::query_as::<_, UserTaskActivity>(
        "SELECT user_id, type_id, define_id, value, gain_value,
                expiry_time, created_at, updated_at
         FROM user_task_activity WHERE user_id = ? AND type_id = ?",
    )
    .bind(user_id)
    .bind(type_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok((updated, true))
}

async fn get_activity(
    pool: &SqlitePool,
    user_id: i64,
    type_id: i32,
) -> sqlx::Result<Option<UserTaskActivity>> {
    sqlx::query_as::<_, UserTaskActivity>(
        "SELECT user_id, type_id, define_id, value, gain_value,
                expiry_time, created_at, updated_at
         FROM user_task_activity
         WHERE user_id = ? AND type_id = ?",
    )
    .bind(user_id)
    .bind(type_id)
    .fetch_optional(pool)
    .await
}

fn empty_activity(user_id: i64, type_id: i32) -> UserTaskActivity {
    UserTaskActivity {
        user_id,
        type_id,
        define_id: 0,
        value: 0,
        gain_value: 0,
        expiry_time: 0,
        created_at: 0,
        updated_at: 0,
    }
}
