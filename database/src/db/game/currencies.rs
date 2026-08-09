use crate::models::game::currencies::Currency;
use common::time::ServerTime;
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::{HashMap, HashSet};

pub const POWER_CURRENCY_ID: i32 = 4;

#[derive(Clone, Copy)]
pub(crate) struct PowerRecovery {
    pub quantity: i32,
    pub last_recover_time: Option<i64>,
    pub limit: i32,
    pub interval_seconds: i32,
    pub amount: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitedExchangeResult {
    Applied,
    InsufficientSource,
    TargetLimit,
    PurchaseLimit,
}

pub struct PowerPurchase {
    pub user_id: i64,
    pub source_currency_id: i32,
    pub cost: i32,
    pub power_currency_id: i32,
    pub power: i32,
    pub power_limit: i32,
    pub expected_purchase_count: i32,
    pub max_purchase_count: i32,
}

pub async fn exchange_with_limit(
    pool: &SqlitePool,
    user_id: i64,
    source_currency_id: i32,
    target_currency_id: i32,
    amount: i32,
    target_limit: i32,
) -> sqlx::Result<LimitedExchangeResult> {
    let mut tx = pool.begin().await?;
    let now = ServerTime::now_ms();
    if !consume_currency_in_transaction(&mut tx, user_id, source_currency_id, amount, now).await? {
        return Ok(LimitedExchangeResult::InsufficientSource);
    }
    if !add_currency_with_limit_in_transaction(
        &mut tx,
        user_id,
        target_currency_id,
        amount,
        target_limit,
        now,
    )
    .await?
    {
        return Ok(LimitedExchangeResult::TargetLimit);
    }
    tx.commit().await?;
    Ok(LimitedExchangeResult::Applied)
}

pub async fn purchase_power(
    pool: &SqlitePool,
    purchase: PowerPurchase,
) -> sqlx::Result<LimitedExchangeResult> {
    let PowerPurchase {
        user_id,
        source_currency_id,
        cost,
        power_currency_id,
        power,
        power_limit,
        expected_purchase_count,
        max_purchase_count,
    } = purchase;
    let mut tx = pool.begin().await?;
    let now = ServerTime::now_ms();
    if !consume_currency_in_transaction(&mut tx, user_id, source_currency_id, cost, now).await? {
        return Ok(LimitedExchangeResult::InsufficientSource);
    }
    if !add_currency_with_limit_in_transaction(
        &mut tx,
        user_id,
        power_currency_id,
        power,
        power_limit,
        now,
    )
    .await?
    {
        return Ok(LimitedExchangeResult::TargetLimit);
    }
    let count = sqlx::query(
        "UPDATE player_state
         SET power_buy_count = power_buy_count + 1
         WHERE player_id = ?
           AND power_buy_count = ?
           AND power_buy_count < ?",
    )
    .bind(user_id)
    .bind(expected_purchase_count)
    .bind(max_purchase_count)
    .execute(&mut *tx)
    .await?;
    if count.rows_affected() != 1 {
        return Ok(LimitedExchangeResult::PurchaseLimit);
    }
    tx.commit().await?;
    Ok(LimitedExchangeResult::Applied)
}

pub async fn consume_currency_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    currency_id: i32,
    amount: i32,
    now: i64,
) -> sqlx::Result<bool> {
    if currency_id == POWER_CURRENCY_ID {
        settle_power_recovery_in_transaction(tx, user_id, now).await?;
    }
    Ok(sqlx::query(
        "UPDATE currencies
         SET quantity = quantity - ?,
             last_recover_time = ?
         WHERE user_id = ? AND currency_id = ? AND quantity >= ?",
    )
    .bind(amount)
    .bind(now)
    .bind(user_id)
    .bind(currency_id)
    .bind(amount)
    .execute(&mut **tx)
    .await?
    .rows_affected()
        == 1)
}

pub(super) async fn add_currency_with_limit_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    currency_id: i32,
    amount: i32,
    limit: i32,
    now: i64,
) -> sqlx::Result<bool> {
    if currency_id == POWER_CURRENCY_ID {
        settle_power_recovery_in_transaction(tx, user_id, now).await?;
    }
    Ok(sqlx::query(
        "INSERT INTO currencies
             (user_id, currency_id, quantity, last_recover_time, expired_time)
         VALUES (?, ?, ?, ?, 0)
         ON CONFLICT(user_id, currency_id) DO UPDATE SET
             quantity = quantity + excluded.quantity,
             last_recover_time = excluded.last_recover_time
         WHERE quantity + excluded.quantity <= ?",
    )
    .bind(user_id)
    .bind(currency_id)
    .bind(amount)
    .bind(now)
    .bind(limit)
    .execute(&mut **tx)
    .await?
    .rows_affected()
        == 1)
}

pub async fn add_currency_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    currency_id: i32,
    amount: i32,
    now: i64,
) -> sqlx::Result<()> {
    if currency_id == POWER_CURRENCY_ID {
        settle_power_recovery_in_transaction(tx, user_id, now).await?;
    }
    sqlx::query(
        "INSERT INTO currencies
             (user_id, currency_id, quantity, last_recover_time, expired_time)
         VALUES (?, ?, ?, ?, 0)
         ON CONFLICT(user_id, currency_id) DO UPDATE SET
             quantity = quantity + excluded.quantity,
             last_recover_time = excluded.last_recover_time",
    )
    .bind(user_id)
    .bind(currency_id)
    .bind(amount)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(super) async fn add_currency_up_to_limit_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    currency_id: i32,
    amount: i32,
    limit: i32,
    now: i64,
) -> sqlx::Result<()> {
    if currency_id == POWER_CURRENCY_ID {
        settle_power_recovery_in_transaction(tx, user_id, now).await?;
    }
    let amount = amount.min(limit).max(0);
    sqlx::query(
        "INSERT INTO currencies
             (user_id, currency_id, quantity, last_recover_time, expired_time)
         VALUES (?, ?, ?, ?, 0)
         ON CONFLICT(user_id, currency_id) DO UPDATE SET
             quantity = MAX(quantity, MIN(quantity + excluded.quantity, ?)),
             last_recover_time = excluded.last_recover_time",
    )
    .bind(user_id)
    .bind(currency_id)
    .bind(amount)
    .bind(now)
    .bind(limit)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn get_currencies(
    pool: &SqlitePool,
    user_id: i64,
    currency_ids: Vec<i32>,
) -> sqlx::Result<Vec<Currency>> {
    if currency_ids.is_empty() {
        return Ok(Vec::new());
    }
    if currency_ids.contains(&POWER_CURRENCY_ID) {
        settle_power_recovery(pool, user_id).await?;
    }

    let placeholders = currency_ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let query = format!(
        "SELECT user_id, currency_id, quantity, last_recover_time, expired_time
         FROM currencies
         WHERE user_id = ? AND currency_id IN ({})",
        placeholders
    );

    let mut q = sqlx::query_as::<_, Currency>(&query).bind(user_id);
    for id in &currency_ids {
        q = q.bind(id);
    }

    let by_id = q
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|currency| (currency.currency_id, currency))
        .collect::<HashMap<_, _>>();

    Ok(currency_ids
        .into_iter()
        .filter_map(|id| by_id.get(&id).cloned())
        .collect())
}

pub async fn get_currency(
    pool: &SqlitePool,
    user_id: i64,
    currency_id: i32,
) -> sqlx::Result<Option<Currency>> {
    if currency_id == POWER_CURRENCY_ID {
        settle_power_recovery(pool, user_id).await?;
    }
    sqlx::query_as::<_, Currency>(
        "SELECT user_id, currency_id, quantity, last_recover_time, expired_time
         FROM currencies
         WHERE user_id = ? AND currency_id = ?",
    )
    .bind(user_id)
    .bind(currency_id)
    .fetch_optional(pool)
    .await
}

pub async fn settle_power_recovery(pool: &SqlitePool, user_id: i64) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    settle_power_recovery_in_transaction(&mut tx, user_id, ServerTime::now_ms()).await?;
    tx.commit().await
}

pub(crate) async fn settle_power_recovery_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    now: i64,
) -> sqlx::Result<()> {
    let Some((quantity, last_recover_time, level)) = sqlx::query_as::<_, (i32, Option<i64>, i32)>(
        "SELECT currencies.quantity, currencies.last_recover_time, users.level
         FROM currencies
         JOIN users ON users.id = currencies.user_id
         WHERE currencies.user_id = ? AND currencies.currency_id = ?",
    )
    .bind(user_id)
    .bind(POWER_CURRENCY_ID)
    .fetch_optional(&mut **tx)
    .await?
    else {
        super::power_maker::settle_in_transaction(tx, user_id, now, false, None).await?;
        return Ok(());
    };

    let tables = config::configs::get();
    let currency = tables
        .currency
        .get(POWER_CURRENCY_ID)
        .ok_or_else(|| sqlx::Error::Protocol("missing power currency config".to_string()))?;
    let recover_limit = tables
        .player_level(level)
        .ok_or_else(|| sqlx::Error::Protocol(format!("missing player level {level}")))?
        .max_auto_recover_power;
    settle_loaded_power(
        tx,
        user_id,
        now,
        PowerRecovery {
            quantity,
            last_recover_time,
            limit: recover_limit,
            interval_seconds: currency.recover_time,
            amount: currency.recover_num,
        },
    )
    .await
}

pub(crate) async fn settle_loaded_power(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    now: i64,
    recovery: PowerRecovery,
) -> sqlx::Result<()> {
    if recovery.quantity >= recovery.limit {
        super::power_maker::settle_in_transaction(tx, user_id, now, true, None).await?;
        return Ok(());
    }

    let Some(last_recover_time) = recovery.last_recover_time else {
        sqlx::query(
            "UPDATE currencies SET last_recover_time = ?
             WHERE user_id = ? AND currency_id = ?",
        )
        .bind(now)
        .bind(user_id)
        .bind(POWER_CURRENCY_ID)
        .execute(&mut **tx)
        .await?;
        super::power_maker::settle_in_transaction(tx, user_id, now, false, None).await?;
        return Ok(());
    };
    let interval = i64::from(recovery.interval_seconds) * 1_000;
    if interval <= 0 || recovery.amount <= 0 {
        super::power_maker::settle_in_transaction(tx, user_id, now, false, None).await?;
        return Ok(());
    }
    let ticks = now.saturating_sub(last_recover_time) / interval;
    if ticks == 0 {
        super::power_maker::settle_in_transaction(tx, user_id, now, false, None).await?;
        return Ok(());
    }

    let recovered = ticks.saturating_mul(i64::from(recovery.amount));
    let recovered_quantity = i64::from(recovery.quantity)
        .saturating_add(recovered)
        .min(i64::from(recovery.limit)) as i32;
    let recovered_at = last_recover_time.saturating_add(ticks.saturating_mul(interval));
    sqlx::query(
        "UPDATE currencies
         SET quantity = ?, last_recover_time = ?
         WHERE user_id = ? AND currency_id = ?",
    )
    .bind(recovered_quantity)
    .bind(recovered_at)
    .bind(user_id)
    .bind(POWER_CURRENCY_ID)
    .execute(&mut **tx)
    .await?;
    let started_at = (recovered_quantity >= recovery.limit).then(|| {
        let missing = i64::from(recovery.limit - recovery.quantity);
        let step = i64::from(recovery.amount);
        let needed_ticks = (missing + step - 1) / step;
        last_recover_time.saturating_add(needed_ticks.saturating_mul(interval))
    });
    super::power_maker::settle_in_transaction(tx, user_id, now, started_at.is_some(), started_at)
        .await?;
    Ok(())
}

pub async fn save_currency(pool: &SqlitePool, currency: &Currency) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    if currency.currency_id == POWER_CURRENCY_ID {
        settle_power_recovery_in_transaction(&mut tx, currency.user_id, ServerTime::now_ms())
            .await?;
    }
    sqlx::query(
        "INSERT INTO currencies (user_id, currency_id, quantity, last_recover_time, expired_time)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(user_id, currency_id) DO UPDATE SET
             quantity = excluded.quantity,
             last_recover_time = excluded.last_recover_time,
             expired_time = excluded.expired_time",
    )
    .bind(currency.user_id)
    .bind(currency.currency_id)
    .bind(currency.quantity)
    .bind(currency.last_recover_time)
    .bind(currency.expired_time)
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

pub async fn add_currency(
    pool: &SqlitePool,
    user_id: i64,
    currency_id: i32,
    amount: i32,
) -> sqlx::Result<()> {
    let timestamp = ServerTime::now_ms();
    let mut tx = pool.begin().await?;
    if currency_id == POWER_CURRENCY_ID {
        settle_power_recovery_in_transaction(&mut tx, user_id, timestamp).await?;
    }

    sqlx::query(
        "INSERT INTO currencies (user_id, currency_id, quantity, last_recover_time, expired_time)
         VALUES (?, ?, ?, ?, 0)
         ON CONFLICT(user_id, currency_id) DO UPDATE SET
             quantity = quantity + excluded.quantity,
             last_recover_time = excluded.last_recover_time",
    )
    .bind(user_id)
    .bind(currency_id)
    .bind(amount)
    .bind(timestamp)
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

pub async fn remove_currency(
    pool: &SqlitePool,
    user_id: i64,
    currency_id: i32,
    amount: i32,
) -> sqlx::Result<bool> {
    let timestamp = ServerTime::now_ms();
    let mut tx = pool.begin().await?;
    if currency_id == POWER_CURRENCY_ID {
        settle_power_recovery_in_transaction(&mut tx, user_id, timestamp).await?;
    }
    let current: Option<i32> =
        sqlx::query_scalar("SELECT quantity FROM currencies WHERE user_id = ? AND currency_id = ?")
            .bind(user_id)
            .bind(currency_id)
            .fetch_optional(&mut *tx)
            .await?;

    if current.unwrap_or(0) < amount {
        return Ok(false);
    }

    sqlx::query(
        "UPDATE currencies
         SET quantity = quantity - ?, last_recover_time = ?
         WHERE user_id = ? AND currency_id = ?",
    )
    .bind(amount)
    .bind(timestamp)
    .bind(user_id)
    .bind(currency_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

pub async fn set_currency(
    pool: &SqlitePool,
    user_id: i64,
    currency_id: i32,
    quantity: i32,
) -> sqlx::Result<()> {
    let timestamp = ServerTime::now_ms();
    let mut tx = pool.begin().await?;
    if currency_id == POWER_CURRENCY_ID {
        settle_power_recovery_in_transaction(&mut tx, user_id, timestamp).await?;
    }

    sqlx::query(
        "INSERT INTO currencies (user_id, currency_id, quantity, last_recover_time, expired_time)
         VALUES (?, ?, ?, ?, 0)
         ON CONFLICT(user_id, currency_id) DO UPDATE SET
             quantity = excluded.quantity,
             last_recover_time = excluded.last_recover_time",
    )
    .bind(user_id)
    .bind(currency_id)
    .bind(quantity)
    .bind(timestamp)
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

pub async fn get_poped_exchange_currency_ids(
    pool: &SqlitePool,
    user_id: i64,
) -> sqlx::Result<HashSet<i32>> {
    let rows: Vec<i32> = sqlx::query_scalar(
        "SELECT currency_id
         FROM currencies
         WHERE user_id = ? AND is_poped = 1",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().collect())
}

pub async fn mark_exchange_currencies_poped(
    pool: &SqlitePool,
    user_id: i64,
    currency_ids: &[i32],
) -> sqlx::Result<()> {
    for currency_id in currency_ids {
        sqlx::query(
            "UPDATE currencies SET is_poped = 1
             WHERE user_id = ? AND currency_id = ?",
        )
        .bind(user_id)
        .bind(currency_id)
        .execute(pool)
        .await?;
    }

    Ok(())
}
