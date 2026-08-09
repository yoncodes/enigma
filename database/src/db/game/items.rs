use crate::models::game::items::{InsightItem, Item, PowerItem};
use common::time::ServerTime;
use sqlx::{Sqlite, SqlitePool, Transaction};

pub async fn consume_item_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    item_id: u32,
    amount: i32,
    now: i64,
) -> sqlx::Result<bool> {
    Ok(sqlx::query(
        "UPDATE items
         SET quantity = quantity - ?,
             last_use_time = ?,
             last_update_time = ?
         WHERE user_id = ? AND item_id = ? AND quantity >= ?",
    )
    .bind(amount)
    .bind(now)
    .bind(now)
    .bind(user_id)
    .bind(item_id)
    .bind(amount)
    .execute(&mut **tx)
    .await?
    .rows_affected()
        == 1)
}

pub async fn add_item_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    item_id: u32,
    amount: i32,
    now: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO items (user_id, item_id, quantity, last_update_time, total_gain_count)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(user_id, item_id) DO UPDATE SET
             quantity = quantity + excluded.quantity,
             last_update_time = excluded.last_update_time,
             total_gain_count = total_gain_count + excluded.total_gain_count",
    )
    .bind(user_id)
    .bind(item_id)
    .bind(amount)
    .bind(now)
    .bind(amount)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

// Items
pub async fn get_all_items(pool: &SqlitePool, user_id: i64) -> sqlx::Result<Vec<Item>> {
    sqlx::query_as("SELECT * FROM items WHERE user_id = ? ORDER BY item_id")
        .bind(user_id)
        .fetch_all(pool)
        .await
}

pub async fn get_item(pool: &SqlitePool, user_id: i64, item_id: u32) -> sqlx::Result<Option<Item>> {
    sqlx::query_as("SELECT * FROM items WHERE user_id = ? AND item_id = ?")
        .bind(user_id)
        .bind(item_id as i64)
        .fetch_optional(pool)
        .await
}

pub async fn save_item(pool: &SqlitePool, item: &Item) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO items (user_id, item_id, quantity, last_use_time, last_update_time, total_gain_count)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(user_id, item_id) DO UPDATE SET
             quantity = excluded.quantity,
             last_use_time = excluded.last_use_time,
             last_update_time = excluded.last_update_time,
             total_gain_count = excluded.total_gain_count"
    )
    .bind(item.user_id)
    .bind(item.item_id)
    .bind(item.quantity)
    .bind(item.last_use_time)
    .bind(item.last_update_time)
    .bind(item.total_gain_count)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn add_item_quantity(
    pool: &SqlitePool,
    user_id: i64,
    item_id: u32,
    amount: i32,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO items (user_id, item_id, quantity, last_update_time, total_gain_count)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(user_id, item_id) DO UPDATE SET
             quantity = quantity + excluded.quantity,
             last_update_time = excluded.last_update_time,
             total_gain_count = total_gain_count + excluded.total_gain_count",
    )
    .bind(user_id)
    .bind(item_id as i64)
    .bind(amount)
    .bind(ServerTime::now_ms())
    .bind(amount as i64)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn remove_item_quantity(
    pool: &SqlitePool,
    user_id: i64,
    item_id: u32,
    amount: i32,
) -> sqlx::Result<bool> {
    let current: Option<i32> =
        sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = ? AND item_id = ?")
            .bind(user_id)
            .bind(item_id as i64)
            .fetch_optional(pool)
            .await?;

    if current.unwrap_or(0) < amount {
        return Ok(false);
    }

    let timestamp = ServerTime::now_ms();
    sqlx::query("UPDATE items SET quantity = quantity - ?, last_use_time = ?, last_update_time = ? WHERE user_id = ? AND item_id = ?")
        .bind(amount)
        .bind(timestamp)
        .bind(timestamp)
        .bind(user_id)
        .bind(item_id as i64)
        .execute(pool)
        .await?;

    Ok(true)
}

pub async fn get_all_power_items(pool: &SqlitePool, user_id: i64) -> sqlx::Result<Vec<PowerItem>> {
    sqlx::query_as(
        r#"

        SELECT
            uid,
            user_id,
            item_id,
            quantity,
            expire_time,
            created_at
        FROM power_items
        WHERE user_id = ?
          AND expire_time = 0
        GROUP BY user_id, item_id

        UNION ALL


        SELECT
            MIN(uid)              AS uid,
            user_id               AS user_id,
            item_id               AS item_id,
            SUM(quantity)         AS quantity,
            MIN(expire_time)      AS expire_time,
            MIN(created_at)       AS created_at
        FROM power_items
        WHERE user_id = ?
          AND expire_time > CAST(strftime('%s','now') AS INTEGER)
        GROUP BY user_id, item_id

        ORDER BY expire_time ASC
        "#,
    )
    .bind(user_id)
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn get_expired_power_items(
    pool: &SqlitePool,
    user_id: i64,
    now_seconds: i64,
) -> sqlx::Result<Vec<PowerItem>> {
    sqlx::query_as(
        "SELECT uid, user_id, item_id, quantity, expire_time, created_at
         FROM power_items
         WHERE user_id = ? AND expire_time > 0 AND expire_time < ?",
    )
    .bind(user_id)
    .bind(now_seconds)
    .fetch_all(pool)
    .await
}

pub async fn convert_expired_power_items(
    pool: &SqlitePool,
    user_id: i64,
    item_uids: &[i64],
    currency_id: i32,
    currency_amount: i32,
) -> sqlx::Result<bool> {
    convert_expired_power_items_up_to_limit(
        pool,
        user_id,
        item_uids,
        currency_id,
        currency_amount,
        i32::MAX,
    )
    .await
}

pub async fn convert_expired_power_items_up_to_limit(
    pool: &SqlitePool,
    user_id: i64,
    item_uids: &[i64],
    currency_id: i32,
    currency_amount: i32,
    currency_limit: i32,
) -> sqlx::Result<bool> {
    let mut tx = pool.begin().await?;
    for uid in item_uids {
        let deleted = sqlx::query(
            "DELETE FROM power_items
             WHERE user_id = ? AND uid = ? AND expire_time > 0
               AND expire_time < ?",
        )
        .bind(user_id)
        .bind(uid)
        .bind(ServerTime::now_ms() / 1000)
        .execute(&mut *tx)
        .await?;
        if deleted.rows_affected() != 1 {
            return Ok(false);
        }
    }
    if currency_amount > 0 {
        super::currencies::add_currency_up_to_limit_in_transaction(
            &mut tx,
            user_id,
            currency_id,
            currency_amount,
            currency_limit,
            ServerTime::now_ms(),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(true)
}

pub async fn get_power_item_by_uid(
    pool: &SqlitePool,
    user_id: i64,
    uid: i64,
) -> sqlx::Result<Option<PowerItem>> {
    sqlx::query_as(
        "SELECT uid, user_id, item_id, quantity, expire_time, created_at
         FROM power_items
         WHERE user_id = ? AND uid = ?",
    )
    .bind(user_id)
    .bind(uid)
    .fetch_optional(pool)
    .await
}

pub async fn consume_power_items_for_currency(
    pool: &SqlitePool,
    user_id: i64,
    uses: &[(i64, i32)],
    currency_id: i32,
    currency_amount: i32,
    currency_limit: i32,
) -> sqlx::Result<Option<Vec<PowerItem>>> {
    let mut tx = pool.begin().await?;
    let mut updates = Vec::with_capacity(uses.len());
    for (uid, amount) in uses {
        let Some(item) = sqlx::query_as::<_, PowerItem>(
            "UPDATE power_items
             SET quantity = quantity - ?
             WHERE user_id = ? AND uid = ? AND quantity >= ?
             RETURNING uid, user_id, item_id, quantity, expire_time, created_at",
        )
        .bind(amount)
        .bind(user_id)
        .bind(uid)
        .bind(amount)
        .fetch_optional(&mut *tx)
        .await?
        else {
            return Ok(None);
        };
        if item.quantity == 0 {
            sqlx::query("DELETE FROM power_items WHERE user_id = ? AND uid = ?")
                .bind(user_id)
                .bind(uid)
                .execute(&mut *tx)
                .await?;
        }
        updates.push(item);
    }

    if !super::currencies::add_currency_with_limit_in_transaction(
        &mut tx,
        user_id,
        currency_id,
        currency_amount,
        currency_limit,
        ServerTime::now_ms(),
    )
    .await?
    {
        return Ok(None);
    }
    tx.commit().await?;
    Ok(Some(updates))
}

pub async fn get_power_item(
    pool: &SqlitePool,
    user_id: i64,
    item_id: u32,
) -> sqlx::Result<Option<PowerItem>> {
    sqlx::query_as("SELECT * FROM power_items WHERE user_id = ? AND item_id = ?")
        .bind(user_id)
        .bind(item_id as i64)
        .fetch_optional(pool)
        .await
}

pub async fn add_power_items(
    pool: &SqlitePool,
    user_id: i64,
    power_items: &[(i32, i32)],
) -> sqlx::Result<Vec<i32>> {
    let mut changed_item_ids = Vec::new();
    let now = common::time::ServerTime::now_ms();
    let game_data = config::configs::get();
    for (item_id, quantity) in power_items {
        let power_item_config = game_data.power_item.get(*item_id);
        let expire_time = if let Some(config) = power_item_config {
            match config.expire_type {
                0 => 0,
                1 => (now / 1000) + (10 * 24 * 60 * 60),
                2 => (now / 1000) + (10 * 24 * 60 * 60),
                3 => (now / 1000) + (10 * 24 * 60 * 60),
                _ => 0,
            }
        } else {
            0
        };
        for _ in 0..*quantity {
            sqlx::query(
                "INSERT INTO power_items (user_id, item_id, quantity, expire_time, created_at)
                 VALUES (?, ?, 1, ?, ?)",
            )
            .bind(user_id)
            .bind(item_id)
            .bind(expire_time)
            .bind(now)
            .execute(pool)
            .await?;
        }
        changed_item_ids.push(*item_id);
        tracing::info!(
            "Added {} power items (id: {}) to user {} with expire_time: {}",
            quantity,
            item_id,
            user_id,
            expire_time
        );
    }
    Ok(changed_item_ids)
}

pub async fn insert_power_item(pool: &SqlitePool, item: &PowerItem) -> sqlx::Result<i64> {
    let result = sqlx::query(
        "INSERT INTO power_items (user_id, item_id, quantity, expire_time, created_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(item.user_id)
    .bind(item.item_id)
    .bind(item.quantity)
    .bind(item.expire_time)
    .bind(item.created_at)
    .execute(pool)
    .await?;
    Ok(result.last_insert_rowid())
}

pub async fn delete_power_item(pool: &SqlitePool, user_id: i64, uid: i64) -> sqlx::Result<bool> {
    let result = sqlx::query("DELETE FROM power_items WHERE user_id = ? AND uid = ?")
        .bind(user_id)
        .bind(uid)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn cleanup_expired_power_items(
    pool: &SqlitePool,
    user_id: Option<i64>,
) -> sqlx::Result<u64> {
    let result = match user_id {
        Some(uid) => sqlx::query(
            "DELETE FROM power_items WHERE user_id = ? AND expire_time <= strftime('%s', 'now')",
        )
        .bind(uid)
        .execute(pool)
        .await?,
        None => {
            sqlx::query("DELETE FROM power_items WHERE expire_time <= strftime('%s', 'now')")
                .execute(pool)
                .await?
        }
    };
    Ok(result.rows_affected())
}

// Insight Items
pub async fn get_all_insight_items(
    pool: &SqlitePool,
    user_id: i64,
) -> sqlx::Result<Vec<InsightItem>> {
    sqlx::query_as("SELECT * FROM insight_items WHERE user_id = ? AND expire_time > strftime('%s', 'now') ORDER BY expire_time")
        .bind(user_id)
        .fetch_all(pool)
        .await
}

pub async fn get_insight_item_by_uid(
    pool: &SqlitePool,
    user_id: i64,
    uid: i64,
) -> sqlx::Result<Option<InsightItem>> {
    sqlx::query_as(
        "SELECT uid, user_id, item_id, quantity, expire_time
         FROM insight_items
         WHERE user_id = ? AND uid = ?",
    )
    .bind(user_id)
    .bind(uid)
    .fetch_optional(pool)
    .await
}

pub async fn consume_insight_item_by_uid(
    pool: &SqlitePool,
    user_id: i64,
    uid: i64,
) -> sqlx::Result<bool> {
    Ok(sqlx::query(
        "UPDATE insight_items
         SET quantity = quantity - 1
         WHERE user_id = ? AND uid = ? AND quantity >= 1",
    )
    .bind(user_id)
    .bind(uid)
    .execute(pool)
    .await?
    .rows_affected()
        == 1)
}

pub async fn add_power_items_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    power_items: &[(i32, i32)],
) -> sqlx::Result<Vec<i32>> {
    let now = ServerTime::now_ms();
    let mut changed = Vec::new();
    for (item_id, amount) in power_items {
        let expire_time = config::configs::get()
            .power_item
            .get(*item_id)
            .map_or(0, |item| match item.expire_type {
                1..=3 => (now / 1000) + 10 * 24 * 60 * 60,
                _ => 0,
            });
        for _ in 0..*amount {
            sqlx::query(
                "INSERT INTO power_items (user_id, item_id, quantity, expire_time, created_at)
                 VALUES (?, ?, 1, ?, ?)",
            )
            .bind(user_id)
            .bind(item_id)
            .bind(expire_time)
            .bind(now)
            .execute(&mut **tx)
            .await?;
        }
        if *amount > 0 {
            changed.push(*item_id);
        }
    }
    Ok(changed)
}

pub async fn add_insight_items_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    insight_items: &[(i32, i32)],
) -> sqlx::Result<Vec<i32>> {
    let now = ServerTime::now_ms();
    let mut changed = Vec::new();
    for (item_id, amount) in insight_items {
        let expire_time = config::configs::get()
            .insight_item
            .get(*item_id)
            .map_or(0, |item| {
                ((now / 1000) + i64::from(item.expire_hours) * 60 * 60) as i32
            });
        for _ in 0..*amount {
            sqlx::query(
                "INSERT INTO insight_items (user_id, item_id, quantity, expire_time)
                 VALUES (?, ?, 1, ?)",
            )
            .bind(user_id)
            .bind(item_id)
            .bind(expire_time)
            .execute(&mut **tx)
            .await?;
        }
        if *amount > 0 {
            changed.push(*item_id);
        }
    }
    Ok(changed)
}

pub async fn get_insight_item(
    pool: &SqlitePool,
    user_id: i64,
    item_id: u32,
) -> sqlx::Result<Option<InsightItem>> {
    sqlx::query_as("SELECT * FROM insight_items WHERE user_id = ? AND item_id = ?")
        .bind(user_id)
        .bind(item_id as i64)
        .fetch_optional(pool)
        .await
}

pub async fn insert_insight_item(pool: &SqlitePool, item: &InsightItem) -> sqlx::Result<i64> {
    let result = sqlx::query(
        "INSERT INTO insight_items (user_id, item_id, quantity, expire_time) VALUES (?, ?, ?, ?)",
    )
    .bind(item.user_id)
    .bind(item.item_id)
    .bind(item.quantity)
    .bind(item.expire_time)
    .execute(pool)
    .await?;
    Ok(result.last_insert_rowid())
}

pub async fn delete_insight_item(pool: &SqlitePool, user_id: i64, uid: i64) -> sqlx::Result<bool> {
    let result = sqlx::query("DELETE FROM insight_items WHERE user_id = ? AND uid = ?")
        .bind(user_id)
        .bind(uid)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn cleanup_expired_insight_items(
    pool: &SqlitePool,
    user_id: Option<i64>,
) -> sqlx::Result<u64> {
    let result = match user_id {
        Some(uid) => sqlx::query(
            "DELETE FROM insight_items WHERE user_id = ? AND expire_time <= strftime('%s', 'now')",
        )
        .bind(uid)
        .execute(pool)
        .await?,
        None => {
            sqlx::query("DELETE FROM insight_items WHERE expire_time <= strftime('%s', 'now')")
                .execute(pool)
                .await?
        }
    };
    Ok(result.rows_affected())
}
