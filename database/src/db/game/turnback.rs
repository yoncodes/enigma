use crate::models::game::turnback::{
    TurnbackDropKind, TurnbackDropState, TurnbackSignInState, TurnbackState,
};
use anyhow::Result;
use sqlx::SqlitePool;

#[derive(Clone, Copy)]
enum TurnbackSignInStateValue {
    HasGet = 2,
}

impl TurnbackSignInStateValue {
    const fn id(self) -> i32 {
        self as i32
    }
}

pub async fn get_active_state(
    pool: &SqlitePool,
    user_id: i64,
    tables: &config::GameDB,
) -> Result<Option<TurnbackState>> {
    let Some(state) = sqlx::query_as::<_, TurnbackState>(
        "SELECT user_id, turnback_id, bonus_point, first_show, has_get_task_bonus,
                sign_in_day, once_bonus, start_time, end_time, remain_addition_count,
                leave_time, month_card_added_buy_count, version, buy_double_bonus,
                get_daily_bonus, has_get_return_reward_ids, return_random_task_id,
                return_left_task_ids, return_last_random_task_id, return_random_refresh_day,
                updated_at
         FROM user_turnback_state
         WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    Ok(tables
        .turnback
        .get(state.turnback_id)
        .is_some()
        .then_some(state))
}

pub async fn get_active_state_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
    tables: &config::GameDB,
) -> Result<Option<TurnbackState>> {
    let state = sqlx::query_as::<_, TurnbackState>(
        "SELECT user_id, turnback_id, bonus_point, first_show, has_get_task_bonus,
                sign_in_day, once_bonus, start_time, end_time, remain_addition_count,
                leave_time, month_card_added_buy_count, version, buy_double_bonus,
                get_daily_bonus, has_get_return_reward_ids, return_random_task_id,
                return_left_task_ids, return_last_random_task_id, return_random_refresh_day,
                updated_at
         FROM user_turnback_state WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(state.filter(|state| tables.turnback.get(state.turnback_id).is_some()))
}

pub async fn ensure_sign_ins(
    pool: &SqlitePool,
    user_id: i64,
    turnback_id: i32,
    tables: &config::GameDB,
) -> Result<Vec<TurnbackSignInState>> {
    let now = common::time::ServerTime::now_ms();

    for row in tables
        .turnback_sign_in
        .iter()
        .filter(|row| row.turnback_id == turnback_id)
    {
        sqlx::query(
            "INSERT INTO user_turnback_sign_ins (user_id, turnback_id, day, updated_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(turnback_id)
        .bind(row.day)
        .bind(now)
        .execute(pool)
        .await?;
    }

    list_sign_ins(pool, user_id, turnback_id).await
}

pub async fn list_sign_ins(
    pool: &SqlitePool,
    user_id: i64,
    turnback_id: i32,
) -> Result<Vec<TurnbackSignInState>> {
    Ok(sqlx::query_as::<_, TurnbackSignInState>(
        "SELECT user_id, turnback_id, day, state, updated_at
         FROM user_turnback_sign_ins
         WHERE user_id = ? AND turnback_id = ?
         ORDER BY day",
    )
    .bind(user_id)
    .bind(turnback_id)
    .fetch_all(pool)
    .await?)
}

pub async fn ensure_drops(
    pool: &SqlitePool,
    user_id: i64,
    turnback_id: i32,
    tables: &config::GameDB,
) -> Result<Vec<TurnbackDropState>> {
    let now = common::time::ServerTime::now_ms();

    for row in tables
        .turnback_drop
        .iter()
        .filter(|row| row.r#type == TurnbackDropKind::Progress.id())
    {
        sqlx::query(
            "INSERT INTO user_turnback_drops (user_id, turnback_id, drop_id, updated_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(turnback_id)
        .bind(row.id)
        .bind(now)
        .execute(pool)
        .await?;
    }

    list_drops(pool, user_id, turnback_id).await
}

pub async fn list_drops(
    pool: &SqlitePool,
    user_id: i64,
    turnback_id: i32,
) -> Result<Vec<TurnbackDropState>> {
    Ok(sqlx::query_as::<_, TurnbackDropState>(
        "SELECT user_id, turnback_id, drop_id, current_num, updated_at
         FROM user_turnback_drops
         WHERE user_id = ? AND turnback_id = ?
         ORDER BY drop_id",
    )
    .bind(user_id)
    .bind(turnback_id)
    .fetch_all(pool)
    .await?)
}

pub async fn mark_first_show(pool: &SqlitePool, user_id: i64, turnback_id: i32) -> Result<bool> {
    update_bool_field(pool, user_id, turnback_id, "first_show").await
}

pub async fn mark_once_bonus(pool: &SqlitePool, user_id: i64, turnback_id: i32) -> Result<bool> {
    update_bool_field(pool, user_id, turnback_id, "once_bonus").await
}

pub async fn mark_once_bonus_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
    turnback_id: i32,
) -> Result<bool> {
    update_bool_field_in_transaction(tx, user_id, turnback_id, "once_bonus").await
}

pub async fn mark_buy_double_bonus(
    pool: &SqlitePool,
    user_id: i64,
    turnback_id: i32,
) -> Result<bool> {
    update_bool_field(pool, user_id, turnback_id, "buy_double_bonus").await
}

pub async fn mark_buy_double_bonus_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
    turnback_id: i32,
) -> Result<bool> {
    update_bool_field_in_transaction(tx, user_id, turnback_id, "buy_double_bonus").await
}

pub async fn claim_sign_in_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
    turnback_id: i32,
    day: i32,
) -> Result<bool> {
    let now = common::time::ServerTime::now_ms();
    let result = sqlx::query(
        "UPDATE user_turnback_sign_ins
         SET state = ?, updated_at = ?
         WHERE user_id = ? AND turnback_id = ? AND day = ? AND state != ?",
    )
    .bind(TurnbackSignInStateValue::HasGet.id())
    .bind(now)
    .bind(user_id)
    .bind(turnback_id)
    .bind(day)
    .bind(TurnbackSignInStateValue::HasGet.id())
    .execute(&mut **tx)
    .await?;
    if result.rows_affected() != 1 {
        return Ok(false);
    }
    sqlx::query(
        "UPDATE user_turnback_state
         SET sign_in_day = max(sign_in_day, ?), updated_at = ?
         WHERE user_id = ? AND turnback_id = ?",
    )
    .bind(day)
    .bind(now)
    .bind(user_id)
    .bind(turnback_id)
    .execute(&mut **tx)
    .await?;
    Ok(true)
}

pub async fn claim_sign_in(
    pool: &SqlitePool,
    user_id: i64,
    turnback_id: i32,
    day: i32,
) -> Result<bool> {
    let now = common::time::ServerTime::now_ms();
    let already_claimed = sqlx::query_scalar::<_, bool>(
        "SELECT state = ? FROM user_turnback_sign_ins
         WHERE user_id = ? AND turnback_id = ? AND day = ?",
    )
    .bind(TurnbackSignInStateValue::HasGet.id())
    .bind(user_id)
    .bind(turnback_id)
    .bind(day)
    .fetch_optional(pool)
    .await?
    .unwrap_or_default();
    if already_claimed {
        return Ok(false);
    }

    sqlx::query(
        "INSERT INTO user_turnback_sign_ins (user_id, turnback_id, day, state, updated_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(user_id, turnback_id, day) DO UPDATE SET
            state = excluded.state,
            updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(turnback_id)
    .bind(day)
    .bind(TurnbackSignInStateValue::HasGet.id())
    .bind(now)
    .execute(pool)
    .await?;

    sqlx::query(
        "UPDATE user_turnback_state
         SET sign_in_day = max(sign_in_day, ?), updated_at = ?
         WHERE user_id = ? AND turnback_id = ?",
    )
    .bind(day)
    .bind(now)
    .bind(user_id)
    .bind(turnback_id)
    .execute(pool)
    .await?;

    Ok(true)
}

pub async fn claim_daily_bonus(
    pool: &SqlitePool,
    user_id: i64,
    turnback_id: i32,
    day: i32,
) -> Result<(i32, bool)> {
    let bit = if (1..=30).contains(&day) {
        1_i32 << (day - 1)
    } else {
        0
    };
    let now = common::time::ServerTime::now_ms();
    let current = sqlx::query_scalar::<_, i32>(
        "SELECT get_daily_bonus FROM user_turnback_state
         WHERE user_id = ? AND turnback_id = ?",
    )
    .bind(user_id)
    .bind(turnback_id)
    .fetch_optional(pool)
    .await?
    .unwrap_or_default();
    if bit == 0 || current & bit != 0 {
        return Ok((current, false));
    }

    sqlx::query(
        "UPDATE user_turnback_state
         SET get_daily_bonus = get_daily_bonus | ?, updated_at = ?
         WHERE user_id = ? AND turnback_id = ?",
    )
    .bind(bit)
    .bind(now)
    .bind(user_id)
    .bind(turnback_id)
    .execute(pool)
    .await?;

    Ok((
        sqlx::query_scalar(
            "SELECT get_daily_bonus FROM user_turnback_state
         WHERE user_id = ? AND turnback_id = ?",
        )
        .bind(user_id)
        .bind(turnback_id)
        .fetch_optional(pool)
        .await?
        .unwrap_or_default(),
        true,
    ))
}

pub async fn claim_daily_bonus_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
    turnback_id: i32,
    day: i32,
) -> Result<(i32, bool)> {
    let bit = if (1..=30).contains(&day) {
        1_i32 << (day - 1)
    } else {
        0
    };
    if bit == 0 {
        return Ok((0, false));
    }
    let result = sqlx::query(
        "UPDATE user_turnback_state
         SET get_daily_bonus = get_daily_bonus | ?, updated_at = ?
         WHERE user_id = ? AND turnback_id = ? AND get_daily_bonus & ? = 0",
    )
    .bind(bit)
    .bind(common::time::ServerTime::now_ms())
    .bind(user_id)
    .bind(turnback_id)
    .bind(bit)
    .execute(&mut **tx)
    .await?;
    let claimed = sqlx::query_scalar(
        "SELECT get_daily_bonus FROM user_turnback_state
         WHERE user_id = ? AND turnback_id = ?",
    )
    .bind(user_id)
    .bind(turnback_id)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or_default();
    Ok((claimed, result.rows_affected() == 1))
}

pub async fn save_claimed_task_bonus(
    pool: &SqlitePool,
    user_id: i64,
    turnback_id: i32,
    ids: &[i32],
) -> Result<Vec<i32>> {
    let now = common::time::ServerTime::now_ms();
    let json = serde_json::to_string(ids)?;

    sqlx::query(
        "UPDATE user_turnback_state
         SET has_get_task_bonus = ?, updated_at = ?
         WHERE user_id = ? AND turnback_id = ?",
    )
    .bind(json)
    .bind(now)
    .bind(user_id)
    .bind(turnback_id)
    .execute(pool)
    .await?;

    Ok(ids.to_vec())
}

pub async fn save_claimed_task_bonus_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
    turnback_id: i32,
    expected_json: &str,
    ids: &[i32],
) -> Result<Option<Vec<i32>>> {
    let json = serde_json::to_string(ids)?;
    let result = sqlx::query(
        "UPDATE user_turnback_state
         SET has_get_task_bonus = ?, updated_at = ?
         WHERE user_id = ? AND turnback_id = ? AND has_get_task_bonus = ?",
    )
    .bind(json)
    .bind(common::time::ServerTime::now_ms())
    .bind(user_id)
    .bind(turnback_id)
    .bind(expected_json)
    .execute(&mut **tx)
    .await?;
    Ok((result.rows_affected() == 1).then(|| ids.to_vec()))
}

pub async fn save_claimed_return_rewards_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
    turnback_id: i32,
    expected_json: &str,
    ids: &[i32],
) -> Result<Option<Vec<i32>>> {
    let json = serde_json::to_string(ids)?;
    let result = sqlx::query(
        "UPDATE user_turnback_state
         SET has_get_return_reward_ids = ?, updated_at = ?
         WHERE user_id = ? AND turnback_id = ? AND has_get_return_reward_ids = ?",
    )
    .bind(json)
    .bind(common::time::ServerTime::now_ms())
    .bind(user_id)
    .bind(turnback_id)
    .bind(expected_json)
    .execute(&mut **tx)
    .await?;
    Ok((result.rows_affected() == 1).then(|| ids.to_vec()))
}

pub async fn save_return_task_rotation(
    pool: &SqlitePool,
    user_id: i64,
    turnback_id: i32,
    expected_active: i32,
    expected_left: &str,
    expected_last: i32,
    expected_refresh_day: i64,
    active: i32,
    left: &[i32],
    last: i32,
    refresh_day: i64,
) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE user_turnback_state
         SET return_random_task_id = ?, return_left_task_ids = ?,
             return_last_random_task_id = ?, return_random_refresh_day = ?, updated_at = ?
         WHERE user_id = ? AND turnback_id = ?
           AND return_random_task_id = ? AND return_left_task_ids = ?
           AND return_last_random_task_id = ? AND return_random_refresh_day = ?",
    )
    .bind(active)
    .bind(serde_json::to_string(left)?)
    .bind(last)
    .bind(refresh_day)
    .bind(common::time::ServerTime::now_ms())
    .bind(user_id)
    .bind(turnback_id)
    .bind(expected_active)
    .bind(expected_left)
    .bind(expected_last)
    .bind(expected_refresh_day)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

async fn update_bool_field(
    pool: &SqlitePool,
    user_id: i64,
    turnback_id: i32,
    field: &'static str,
) -> Result<bool> {
    let sql = format!(
        "UPDATE user_turnback_state
         SET {field} = 1, updated_at = ?
         WHERE user_id = ? AND turnback_id = ? AND {field} = 0"
    );

    let result = sqlx::query(&sql)
        .bind(common::time::ServerTime::now_ms())
        .bind(user_id)
        .bind(turnback_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() != 0)
}

async fn update_bool_field_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
    turnback_id: i32,
    field: &'static str,
) -> Result<bool> {
    let sql = format!(
        "UPDATE user_turnback_state
         SET {field} = 1, updated_at = ?
         WHERE user_id = ? AND turnback_id = ? AND {field} = 0"
    );
    let result = sqlx::query(&sql)
        .bind(common::time::ServerTime::now_ms())
        .bind(user_id)
        .bind(turnback_id)
        .execute(&mut **tx)
        .await?;
    Ok(result.rows_affected() != 0)
}
