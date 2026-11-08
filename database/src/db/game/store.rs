use sqlx::SqlitePool;
use std::collections::HashMap;

pub async fn get_buy_counts(pool: &SqlitePool, user_id: i64) -> sqlx::Result<HashMap<i32, i32>> {
    let rows = sqlx::query_as::<_, (i32, i32)>(
        "SELECT goods_id, buy_count
         FROM user_store_goods
         WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().collect())
}

pub async fn add_buy_count(
    pool: &SqlitePool,
    user_id: i64,
    goods_id: i32,
    count: i32,
) -> sqlx::Result<i32> {
    sqlx::query(
        "INSERT INTO user_store_goods (user_id, goods_id, buy_count)
         VALUES (?, ?, ?)
         ON CONFLICT(user_id, goods_id) DO UPDATE SET
             buy_count = buy_count + excluded.buy_count",
    )
    .bind(user_id)
    .bind(goods_id)
    .bind(count)
    .execute(pool)
    .await?;

    let buy_count = sqlx::query_scalar(
        "SELECT buy_count FROM user_store_goods
         WHERE user_id = ? AND goods_id = ?",
    )
    .bind(user_id)
    .bind(goods_id)
    .fetch_one(pool)
    .await?;

    Ok(buy_count)
}

pub async fn add_buy_count_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
    goods_id: i32,
    expected_count: i32,
    count: i32,
) -> sqlx::Result<Option<i32>> {
    let result = if expected_count == 0 {
        sqlx::query(
            "INSERT INTO user_store_goods (user_id, goods_id, buy_count)
             VALUES (?, ?, ?)
             ON CONFLICT(user_id, goods_id) DO UPDATE SET
                 buy_count = user_store_goods.buy_count + excluded.buy_count
             WHERE user_store_goods.buy_count = 0",
        )
        .bind(user_id)
        .bind(goods_id)
        .bind(count)
        .execute(&mut **tx)
        .await?
    } else {
        sqlx::query(
            "UPDATE user_store_goods SET buy_count = buy_count + ?
             WHERE user_id = ? AND goods_id = ? AND buy_count = ?",
        )
        .bind(count)
        .bind(user_id)
        .bind(goods_id)
        .bind(expected_count)
        .execute(&mut **tx)
        .await?
    };
    if result.rows_affected() != 1 {
        return Ok(None);
    }
    Ok(Some(expected_count + count))
}

#[cfg(test)]
mod tests;
