use crate::models::game::mail::UserMail;
use common::time::ServerTime;
use sqlx::SqlitePool;

pub async fn expire(pool: &SqlitePool, user_id: i64) -> sqlx::Result<()> {
    let now = ServerTime::now_ms();

    sqlx::query(
        "INSERT INTO user_mail_history
         (user_id, mail_incr_id, mail_id, attachment, action, action_time, state_at_action)
         SELECT user_id, incr_id, mail_id, attachment, 'expired', ?, state
         FROM user_mails
         WHERE user_id = ? AND expire_time > 0 AND expire_time < ?",
    )
    .bind(now)
    .bind(user_id)
    .bind(now)
    .execute(pool)
    .await?;

    sqlx::query("DELETE FROM user_mails WHERE user_id = ? AND expire_time > 0 AND expire_time < ?")
        .bind(user_id)
        .bind(now)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn get_all(pool: &SqlitePool, user_id: i64) -> sqlx::Result<Vec<UserMail>> {
    expire(pool, user_id).await?;

    sqlx::query_as::<_, UserMail>(
        "SELECT incr_id, user_id, mail_id, params, attachment, state, create_time,
                sender, title, content, copy, expire_time, sender_type, jump_title, jump, is_lock
         FROM user_mails
         WHERE user_id = ?
         ORDER BY create_time DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn get_created_since(
    pool: &SqlitePool,
    user_id: i64,
    since: i64,
) -> sqlx::Result<Vec<UserMail>> {
    expire(pool, user_id).await?;

    sqlx::query_as::<_, UserMail>(
        "SELECT incr_id, user_id, mail_id, params, attachment, state, create_time,
                sender, title, content, copy, expire_time, sender_type, jump_title, jump, is_lock
         FROM user_mails
         WHERE user_id = ? AND create_time > ?
         ORDER BY create_time",
    )
    .bind(user_id)
    .bind(since)
    .fetch_all(pool)
    .await
}

pub async fn unread_count(pool: &SqlitePool, user_id: i64) -> sqlx::Result<i64> {
    expire(pool, user_id).await?;

    sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM user_mails
         WHERE user_id = ? AND state = 0 AND (expire_time = 0 OR expire_time > ?)",
    )
    .bind(user_id)
    .bind(ServerTime::now_ms())
    .fetch_one(pool)
    .await
}

pub async fn get_claimable(pool: &SqlitePool, user_id: i64) -> sqlx::Result<Vec<UserMail>> {
    expire(pool, user_id).await?;

    sqlx::query_as::<_, UserMail>(
        "SELECT incr_id, user_id, mail_id, params, attachment, state, create_time,
                sender, title, content, copy, expire_time, sender_type, jump_title, jump, is_lock
         FROM user_mails
         WHERE user_id = ? AND state = 0
         ORDER BY create_time DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn get_by_incr_id(
    pool: &SqlitePool,
    user_id: i64,
    incr_id: i64,
) -> sqlx::Result<Option<UserMail>> {
    expire(pool, user_id).await?;

    sqlx::query_as::<_, UserMail>(
        "SELECT incr_id, user_id, mail_id, params, attachment, state, create_time,
                sender, title, content, copy, expire_time, sender_type, jump_title, jump, is_lock
         FROM user_mails
         WHERE user_id = ? AND incr_id = ?",
    )
    .bind(user_id)
    .bind(incr_id)
    .fetch_optional(pool)
    .await
}

pub async fn mark_claimed(pool: &SqlitePool, user_id: i64, incr_ids: Vec<i64>) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    mark_claimed_in_transaction(&mut tx, user_id, &incr_ids).await?;
    tx.commit().await
}

pub async fn mark_claimed_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
    incr_ids: &[i64],
) -> sqlx::Result<()> {
    let now = ServerTime::now_ms();

    for incr_id in incr_ids {
        let result = sqlx::query(
            "UPDATE user_mails SET state = 1
             WHERE user_id = ? AND incr_id = ? AND state != 1",
        )
        .bind(user_id)
        .bind(incr_id)
        .execute(&mut **tx)
        .await?;
        if result.rows_affected() != 1 {
            return Err(sqlx::Error::RowNotFound);
        }

        sqlx::query(
            "INSERT INTO user_mail_history
             (user_id, mail_incr_id, mail_id, attachment, action, action_time, state_at_action)
             SELECT user_id, incr_id, mail_id, attachment, 'claimed', ?, 1
             FROM user_mails
             WHERE user_id = ? AND incr_id = ?",
        )
        .bind(now)
        .bind(user_id)
        .bind(incr_id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

pub async fn set_lock(
    pool: &SqlitePool,
    user_id: i64,
    incr_id: i64,
    lock: bool,
) -> sqlx::Result<bool> {
    let result = sqlx::query("UPDATE user_mails SET is_lock = ? WHERE user_id = ? AND incr_id = ?")
        .bind(lock)
        .bind(user_id)
        .bind(incr_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() == 1)
}

pub async fn delete_claimed_unlocked(pool: &SqlitePool, user_id: i64) -> sqlx::Result<Vec<i64>> {
    let mut transaction = pool.begin().await?;
    let incr_ids = sqlx::query_scalar::<_, i64>(
        "SELECT incr_id FROM user_mails
         WHERE user_id = ? AND state = 1 AND is_lock = 0
         ORDER BY create_time DESC",
    )
    .bind(user_id)
    .fetch_all(&mut *transaction)
    .await?;

    if incr_ids.is_empty() {
        transaction.commit().await?;
        return Ok(incr_ids);
    }

    let now = ServerTime::now_ms();
    sqlx::query(
        "INSERT INTO user_mail_history
         (user_id, mail_incr_id, mail_id, attachment, action, action_time, state_at_action)
         SELECT user_id, incr_id, mail_id, attachment, 'deleted', ?, state
         FROM user_mails
         WHERE user_id = ? AND state = 1 AND is_lock = 0",
    )
    .bind(now)
    .bind(user_id)
    .execute(&mut *transaction)
    .await?;

    sqlx::query("DELETE FROM user_mails WHERE user_id = ? AND state = 1 AND is_lock = 0")
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;

    Ok(incr_ids)
}

pub async fn mark_jump(pool: &SqlitePool, user_id: i64, incr_id: i64) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "INSERT INTO user_mail_history
         (user_id, mail_incr_id, mail_id, attachment, action, action_time, state_at_action)
         SELECT user_id, incr_id, mail_id, attachment, 'jumped', ?, state
         FROM user_mails
         WHERE user_id = ? AND incr_id = ?",
    )
    .bind(ServerTime::now_ms())
    .bind(user_id)
    .bind(incr_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() == 1)
}
