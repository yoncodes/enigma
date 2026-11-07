use crate::{error::AppError, reward};
use sonettobuf::{
    Act243EpisodeRecord, Act243GiftRecord, Act243GrowthRecord, Act243InfoPush,
    Act243ReportEpisodeReply, GetAct243InfoReply,
};
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};

mod growth;
mod report;

pub use growth::{Act243Cost, unlock_racer, upgrade_gift};
pub use report::{Act243Report, report_episode};

const ACTIVITY_TYPE_ID: i32 = 243;

type EpisodeState = (i32, i32, i32, String, String);

pub async fn info(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
) -> Result<GetAct243InfoReply, AppError> {
    validate_activity(activity_id)?;
    let states = sqlx::query_as::<_, (i32, i32, i32, i32, String, String)>(
        "SELECT episode_id, best_score, best_star, best_time_ms, claimed_stars, record
         FROM user_activity243_episodes
         WHERE user_id = ? AND activity_id = ?",
    )
    .bind(player_id)
    .bind(activity_id)
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|(episode_id, score, star, time, claimed, record)| {
        (episode_id, (score, star, time, claimed, record))
    })
    .collect::<HashMap<_, _>>();

    let mut episodes = config::configs::get()
        .activity243_episode
        .iter()
        .filter(|row| row.activity_id == activity_id)
        .map(|row| episode_record(row, states.get(&row.episode_id)))
        .collect::<Result<Vec<_>, AppError>>()?;
    episodes.sort_by_key(|episode| episode.episode_id.unwrap_or_default());

    Ok(GetAct243InfoReply {
        activity_id: Some(activity_id),
        episodes,
        growth: Some(growth(db, player_id, activity_id).await?),
    })
}

pub async fn info_push(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
) -> Result<Act243InfoPush, AppError> {
    let info = info(db, player_id, activity_id).await?;
    Ok(Act243InfoPush {
        activity_id: info.activity_id,
        episodes: info.episodes,
        growth: info.growth,
    })
}

async fn growth(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
) -> Result<Act243GrowthRecord, AppError> {
    let mut gifts = sqlx::query_as::<_, (i32, i32)>(
        "SELECT gift_point, level
         FROM user_activity243_gifts
         WHERE user_id = ? AND activity_id = ?",
    )
    .bind(player_id)
    .bind(activity_id)
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|(gift_point, level)| Act243GiftRecord {
        gift_point: Some(gift_point),
        level: Some(level),
    })
    .collect::<Vec<_>>();
    gifts.sort_by_key(|gift| gift.gift_point.unwrap_or_default());

    let mut unlocked = sqlx::query_scalar::<_, i32>(
        "SELECT racer_id FROM user_activity243_racers
         WHERE user_id = ? AND activity_id = ?",
    )
    .bind(player_id)
    .bind(activity_id)
    .fetch_all(db)
    .await?
    .into_iter()
    .collect::<HashSet<_>>();
    if has_racing_progression(activity_id) {
        unlocked.extend(
            config::configs::get()
                .racing_racer
                .iter()
                .filter(|racer| racer.display == 1 && racer.cost == 0)
                .map(|racer| racer.id),
        );
    }
    let mut unlocked_racers = unlocked.into_iter().collect::<Vec<_>>();
    unlocked_racers.sort_unstable();

    Ok(Act243GrowthRecord {
        gifts,
        unlocked_racers,
    })
}

fn episode_record(
    row: &config::activity243_episode::Activity243Episode,
    state: Option<&EpisodeState>,
) -> Result<Act243EpisodeRecord, AppError> {
    let claimed_stars = state
        .map(|state| serde_json::from_str(&state.3))
        .transpose()?
        .unwrap_or_default();
    Ok(Act243EpisodeRecord {
        episode_id: Some(row.episode_id),
        episode_type: Some(row.episode_type),
        is_finished: Some(state.is_some()),
        best_score: Some(state.map(|state| state.0).unwrap_or_default()),
        best_star: Some(state.map(|state| state.1).unwrap_or_default()),
        best_time_ms: Some(state.map(|state| state.2).unwrap_or_default()),
        claimed_stars,
        record: Some(state.map(|state| state.4.clone()).unwrap_or_default()),
    })
}

fn validate_activity(activity_id: i32) -> Result<(), AppError> {
    let tables = config::configs::get();
    if tables
        .activity
        .get(activity_id)
        .is_some_and(|activity| activity.type_id == ACTIVITY_TYPE_ID)
        && tables
            .activity243_episode
            .iter()
            .any(|episode| episode.activity_id == activity_id)
    {
        Ok(())
    } else {
        Err(AppError::InvalidRequest)
    }
}

fn has_racing_progression(activity_id: i32) -> bool {
    config::configs::get()
        .racing_gift
        .iter()
        .any(|gift| gift.activity_id == activity_id)
}

fn currency_id(activity_id: i32) -> Result<i32, AppError> {
    config::configs::get()
        .activity243_const
        .iter()
        .find(|row| row.activity_id == activity_id && row.id == 1)
        .and_then(|row| row.value.parse().ok())
        .filter(|id| *id > 0)
        .ok_or(AppError::InvalidRequest)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
