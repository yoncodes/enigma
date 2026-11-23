use anyhow::Result;
use sqlx::{Sqlite, SqlitePool, Transaction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity236State {
    pub score: i32,
    pub gain_reward_ids: Vec<i32>,
}

pub async fn get_or_create_state(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
) -> Result<Activity236State> {
    sqlx::query(
        "INSERT OR IGNORE INTO user_activity236_state (user_id, activity_id)
         VALUES (?, ?)",
    )
    .bind(user_id)
    .bind(activity_id)
    .execute(pool)
    .await?;

    get_state(pool, user_id, activity_id).await
}

pub async fn get_state(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
) -> Result<Activity236State> {
    let (score, reward_ids) = sqlx::query_as::<_, (i32, String)>(
        "SELECT score, gain_reward_ids
         FROM user_activity236_state
         WHERE user_id = ? AND activity_id = ?",
    )
    .bind(user_id)
    .bind(activity_id)
    .fetch_one(pool)
    .await?;

    Ok(Activity236State {
        score,
        gain_reward_ids: serde_json::from_str(&reward_ids)?,
    })
}

async fn get_state_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    activity_id: i32,
) -> Result<Activity236State> {
    let (score, reward_ids) = sqlx::query_as::<_, (i32, String)>(
        "SELECT score, gain_reward_ids
         FROM user_activity236_state
         WHERE user_id = ? AND activity_id = ?",
    )
    .bind(user_id)
    .bind(activity_id)
    .fetch_one(&mut **tx)
    .await?;

    Ok(Activity236State {
        score,
        gain_reward_ids: serde_json::from_str(&reward_ids)?,
    })
}

pub async fn increment_score_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    activity_id: i32,
    score_delta: i32,
) -> Result<Activity236State> {
    sqlx::query(
        "INSERT OR IGNORE INTO user_activity236_state (user_id, activity_id)
         VALUES (?, ?)",
    )
    .bind(user_id)
    .bind(activity_id)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        "UPDATE user_activity236_state
         SET score = score + ?
         WHERE user_id = ? AND activity_id = ?",
    )
    .bind(score_delta)
    .bind(user_id)
    .bind(activity_id)
    .execute(&mut **tx)
    .await?;

    get_state_in_transaction(tx, user_id, activity_id).await
}

pub async fn claim_rewards_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    activity_id: i32,
    expected: &Activity236State,
    gain_reward_ids: &[i32],
) -> Result<bool> {
    let expected_ids = serde_json::to_string(&expected.gain_reward_ids)?;
    let gain_reward_ids = serde_json::to_string(gain_reward_ids)?;
    let changed = sqlx::query(
        "UPDATE user_activity236_state
         SET gain_reward_ids = ?
         WHERE user_id = ? AND activity_id = ?
           AND score = ? AND gain_reward_ids = ?",
    )
    .bind(gain_reward_ids)
    .bind(user_id)
    .bind(activity_id)
    .bind(expected.score)
    .bind(expected_ids)
    .execute(&mut **tx)
    .await?
    .rows_affected();

    Ok(changed == 1)
}

#[cfg(test)]
mod tests;
