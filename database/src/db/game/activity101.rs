use anyhow::Result;
use sqlx::SqlitePool;

use super::activity_state::{self, ActivityStateKind, ActivityStateSet};

/// Get activity 101 info for a user
pub async fn get_activity101_info(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
) -> Result<(Vec<(i32, i32)>, i32, bool)> {
    let states =
        activity_state::get(pool, user_id, activity_id, ActivityStateKind::Act101Day).await?;

    let login_count = sqlx::query_scalar::<_, i32>(
        "SELECT addup_sign_in_day FROM user_sign_in_info WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .unwrap_or(0);

    let once =
        activity_state::get(pool, user_id, activity_id, ActivityStateKind::Act101Once).await?;
    let got_once_bonus = once.get(&0).is_some_and(|(state, _, _)| *state == 2);

    let mut days = config::configs::get()
        .activity101
        .iter()
        .filter(|row| row.activity_id == activity_id)
        .map(|row| row.id)
        .collect::<Vec<_>>();
    days.sort_unstable();

    let infos = days
        .into_iter()
        .map(|day| {
            let stored = states.get(&day).map(|(state, _, _)| *state).unwrap_or(0);
            let state = if stored == 2 {
                2
            } else if day <= login_count {
                1
            } else {
                0
            };

            (day, state)
        })
        .collect();

    Ok((infos, login_count, got_once_bonus))
}

/// Claim a day's reward
pub async fn claim_activity101_day(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
    day_id: i32,
) -> Result<bool> {
    let (infos, _, _) = get_activity101_info(pool, user_id, activity_id).await?;
    let Some((_, state)) = infos.into_iter().find(|(day, _)| *day == day_id) else {
        return Ok(false);
    };

    if state != 1 {
        return Ok(false);
    }

    activity_state::set(
        pool,
        user_id,
        activity_id,
        ActivityStateSet {
            kind: ActivityStateKind::Act101Day,
            entry_id: day_id,
            state: 2,
            progress: 0,
            ext: "",
        },
    )
    .await?;
    Ok(true)
}

pub async fn claim_activity101_day_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
    activity_id: i32,
    day_id: i32,
) -> Result<bool> {
    let login_count = sqlx::query_scalar::<_, i32>(
        "SELECT addup_sign_in_day FROM user_sign_in_info WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or_default();
    if day_id <= 0 || day_id > login_count {
        return Ok(false);
    }
    let stored_state = sqlx::query_scalar::<_, i32>(
        "SELECT state
         FROM user_activity_state
         WHERE user_id = ? AND activity_id = ? AND kind = ? AND entry_id = ?",
    )
    .bind(user_id)
    .bind(activity_id)
    .bind(ActivityStateKind::Act101Day.id())
    .bind(day_id)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or_default();
    if stored_state == 2 {
        return Ok(false);
    }

    activity_state::transition_in_transaction(
        tx,
        user_id,
        activity_id,
        stored_state,
        ActivityStateSet {
            kind: ActivityStateKind::Act101Day,
            entry_id: day_id,
            state: 2,
            progress: 0,
            ext: "",
        },
    )
    .await
}
