use anyhow::Result;
use common::time::ServerTime;
use sonettobuf::PowerItem;
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::BTreeMap;

const POWER_ITEM_ID: i32 = 31;
const PRODUCTION_SECONDS: i32 = 12 * 60 * 60;
const EXPIRY_DAYS: i64 = 7;
const DAY_MS: i64 = 24 * 60 * 60 * 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PowerMakerState {
    pub status: i32,
    pub next_remain_second: i32,
    pub make_count: i32,
    pub logout_second: i32,
}

pub async fn get_state(pool: &SqlitePool, user_id: i64) -> Result<PowerMakerState> {
    let state = sqlx::query_as::<_, (i32, i32, i32, i32)>(
        "SELECT status, next_remain_second, make_count, logout_second
         FROM user_power_maker_state
         WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(state.map_or_else(default_state, state_from_row))
}

pub async fn take_state(
    pool: &SqlitePool,
    user_id: i64,
    is_login: bool,
) -> Result<PowerMakerState> {
    let mut tx = pool.begin().await?;
    let now = ServerTime::now_ms();
    super::currencies::settle_power_recovery_in_transaction(&mut tx, user_id, now).await?;
    let row = sqlx::query_as::<_, (i32, i32, i32, i32, i64)>(
        "SELECT status, next_remain_second, make_count, logout_second, last_logout_at
         FROM user_power_maker_state
         WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;

    let state = if let Some((status, next, make_count, stored_logout, last_logout_at)) = row {
        let logout_second = if is_login && last_logout_at > 0 {
            seconds_between(last_logout_at, now)
        } else if is_login {
            stored_logout
        } else {
            0
        };
        let state = PowerMakerState {
            status,
            next_remain_second: next,
            make_count: if is_login { make_count } else { 0 },
            logout_second,
        };
        if is_login {
            sqlx::query(
                "UPDATE user_power_maker_state
                 SET make_count = 0, logout_second = 0, last_logout_at = 0
                 WHERE user_id = ?",
            )
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        }
        state
    } else {
        default_state()
    };
    tx.commit().await?;
    Ok(state)
}

pub async fn record_logout(pool: &SqlitePool, user_id: i64) -> Result<()> {
    let mut tx = pool.begin().await?;
    let now = ServerTime::now_ms();
    super::currencies::settle_power_recovery_in_transaction(&mut tx, user_id, now).await?;
    sqlx::query(
        "UPDATE user_power_maker_state
         SET make_count = 0, logout_second = 0, last_logout_at = ?
         WHERE user_id = ?",
    )
    .bind(now)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

pub(crate) async fn settle_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    now: i64,
    making: bool,
    started_at: Option<i64>,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO user_power_maker_state
             (user_id, status, next_remain_second, updated_at)
         VALUES (?, 0, ?, ?)
         ON CONFLICT(user_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(PRODUCTION_SECONDS)
    .bind(now)
    .execute(&mut **tx)
    .await?;

    let (stored_next, updated_at): (i32, i64) = sqlx::query_as(
        "SELECT next_remain_second, updated_at
         FROM user_power_maker_state WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await?;
    let next = normalize_next(stored_next);
    if updated_at <= 0 || !making {
        update_progress(tx, user_id, making, next, now, 0).await?;
        return Ok(());
    }

    let active_since = started_at.unwrap_or(updated_at).max(updated_at).min(now);
    let elapsed = seconds_between(active_since, now);
    let settled_at = active_since.saturating_add(i64::from(elapsed) * 1_000);
    if elapsed < next {
        update_progress(tx, user_id, true, next - elapsed, settled_at, 0).await?;
        return Ok(());
    }

    let after_first = elapsed - next;
    let produced = 1 + after_first / PRODUCTION_SECONDS;
    let remainder = after_first % PRODUCTION_SECONDS;
    let new_next = PRODUCTION_SECONDS - remainder;
    let first_produced_at = active_since + i64::from(next) * 1_000;
    let mut batches = BTreeMap::<i64, (i32, i64)>::new();
    for index in 0..produced {
        let produced_at = first_produced_at
            .saturating_add(i64::from(index).saturating_mul(i64::from(PRODUCTION_SECONDS) * 1_000));
        let expire_time = expiry_time(produced_at);
        let batch = batches.entry(expire_time).or_insert((0, produced_at));
        batch.0 += 1;
    }
    for (expire_time, (quantity, created_at)) in batches {
        upsert_batch(tx, user_id, quantity, expire_time, created_at).await?;
    }
    update_progress(tx, user_id, true, new_next, settled_at, produced).await
}

async fn update_progress(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    making: bool,
    next: i32,
    now: i64,
    produced: i32,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE user_power_maker_state
         SET status = ?, next_remain_second = ?,
             make_count = make_count + ?, updated_at = ?
         WHERE user_id = ?",
    )
    .bind(i32::from(making))
    .bind(next)
    .bind(produced)
    .bind(now)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn upsert_batch(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    quantity: i32,
    expire_time: i64,
    created_at: i64,
) -> sqlx::Result<()> {
    let updated = sqlx::query(
        "UPDATE power_items SET quantity = quantity + ?
         WHERE uid = (
             SELECT uid FROM power_items
             WHERE user_id = ? AND item_id = ? AND expire_time = ?
             ORDER BY uid LIMIT 1
         )",
    )
    .bind(quantity)
    .bind(user_id)
    .bind(POWER_ITEM_ID)
    .bind(expire_time)
    .execute(&mut **tx)
    .await?;
    if updated.rows_affected() == 0 {
        sqlx::query(
            "INSERT INTO power_items
                 (user_id, item_id, quantity, expire_time, created_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(POWER_ITEM_ID)
        .bind(quantity)
        .bind(expire_time)
        .bind(created_at)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

fn expiry_time(produced_at: i64) -> i64 {
    (ServerTime::server_day_start_ms(produced_at) + EXPIRY_DAYS * DAY_MS) / 1_000
}

fn seconds_between(earlier: i64, later: i64) -> i32 {
    (later.saturating_sub(earlier) / 1_000).min(i64::from(i32::MAX)) as i32
}

fn normalize_next(next: i32) -> i32 {
    if next > 0 {
        next.min(PRODUCTION_SECONDS)
    } else {
        PRODUCTION_SECONDS
    }
}

fn state_from_row(
    (status, next, make_count, logout_second): (i32, i32, i32, i32),
) -> PowerMakerState {
    PowerMakerState {
        status,
        next_remain_second: next,
        make_count,
        logout_second,
    }
}

fn default_state() -> PowerMakerState {
    PowerMakerState {
        status: 0,
        next_remain_second: PRODUCTION_SECONDS,
        make_count: 0,
        logout_second: 0,
    }
}

pub async fn get_maker_items(pool: &SqlitePool, user_id: i64) -> Result<Vec<PowerItem>> {
    let items = sqlx::query_as::<_, crate::models::game::items::PowerItem>(
        "SELECT uid, user_id, item_id, quantity, expire_time, created_at
         FROM power_items
         WHERE user_id = ? AND item_id = ?
           AND (expire_time = 0 OR expire_time > CAST(strftime('%s','now') AS INTEGER))
         ORDER BY expire_time, uid",
    )
    .bind(user_id)
    .bind(POWER_ITEM_ID)
    .fetch_all(pool)
    .await?;

    Ok(items.into_iter().map(Into::into).collect())
}

#[cfg(test)]
mod tests;
