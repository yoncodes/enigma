use super::*;
use common::time::ServerTime;

pub struct Act243Report {
    pub reply: Act243ReportEpisodeReply,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
}

pub async fn report_episode(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
    episode_id: i32,
    score: i32,
    star: i32,
    time_ms: i32,
    record: String,
) -> Result<Act243Report, AppError> {
    validate_activity(activity_id)?;
    if score < 0 || star < 0 || time_ms < 0 {
        return Err(AppError::InvalidRequest);
    }
    let episode = config::configs::get()
        .activity243_episode
        .iter()
        .find(|row| row.activity_id == activity_id && row.episode_id == episode_id)
        .ok_or(AppError::InvalidRequest)?;
    let mut tx = db.begin().await?;
    if episode.pre_episode_id > 0
        && sqlx::query_scalar::<_, i32>(
            "SELECT 1 FROM user_activity243_episodes
             WHERE user_id = ? AND activity_id = ? AND episode_id = ?",
        )
        .bind(player_id)
        .bind(activity_id)
        .bind(episode.pre_episode_id)
        .fetch_optional(&mut *tx)
        .await?
        .is_none()
    {
        return Err(AppError::InvalidRequest);
    }

    let saved = sqlx::query_as::<_, EpisodeState>(
        "SELECT best_score, best_star, best_time_ms, claimed_stars, record
         FROM user_activity243_episodes
         WHERE user_id = ? AND activity_id = ? AND episode_id = ?",
    )
    .bind(player_id)
    .bind(activity_id)
    .bind(episode_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (old_score, old_star, old_time, claimed_json, old_record) =
        saved
            .clone()
            .unwrap_or((0, 0, 0, "[]".into(), String::new()));
    let best_score = old_score.max(score);
    let mut best_star = old_star.max(star);
    let best_time_ms = match (old_time, time_ms) {
        (0, time) | (time, 0) => time,
        (old, new) => old.min(new),
    };
    let racing_record = if has_racing_progression(activity_id) {
        let (record, incoming_star, merged_star) = merge_record(&old_record, &record)?;
        if incoming_star != star {
            return Err(AppError::InvalidRequest);
        }
        best_star = merged_star;
        Some(record)
    } else {
        None
    };
    let is_new_best = saved.is_none()
        || best_score != old_score
        || best_star != old_star
        || best_time_ms != old_time;
    let record = if let Some(record) = racing_record {
        record
    } else if is_new_best {
        record
    } else {
        old_record
    };

    let mut claimed_stars = serde_json::from_str::<Vec<i32>>(&claimed_json)?;
    claimed_stars.sort_unstable();
    claimed_stars.dedup();
    let claimed = claimed_stars.iter().copied().collect::<HashSet<_>>();
    let mut new_star_rewards = config::configs::get()
        .racing_reward
        .iter()
        .filter(|row| row.episode_id == episode_id && row.star <= best_star)
        .filter(|row| !claimed.contains(&row.star))
        .collect::<Vec<_>>();
    new_star_rewards.sort_by_key(|row| row.star);
    claimed_stars.extend(new_star_rewards.iter().map(|row| row.star));

    let mut reward_set = reward::RewardSet::default();
    if episode.clear_bonus > 0 {
        reward_set.extend(reward::parse_reward_id(episode.clear_bonus));
    }
    for row in new_star_rewards {
        reward_set.extend(reward::parse_reward_id(row.bonus));
    }
    let material_changes = reward_set.material_changes();
    let rewards = reward::RewardManager::new(player_id)
        .apply_in_transaction(&mut tx, db, reward_set)
        .await?;
    let now = ServerTime::now_ms();
    sqlx::query(
        "INSERT INTO user_activity243_episodes
            (user_id, activity_id, episode_id, best_score, best_star, best_time_ms,
             claimed_stars, record, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(user_id, activity_id, episode_id) DO UPDATE SET
            best_score = excluded.best_score,
            best_star = excluded.best_star,
            best_time_ms = excluded.best_time_ms,
            claimed_stars = excluded.claimed_stars,
            record = excluded.record,
            updated_at = excluded.updated_at",
    )
    .bind(player_id)
    .bind(activity_id)
    .bind(episode_id)
    .bind(best_score)
    .bind(best_star)
    .bind(best_time_ms)
    .bind(serde_json::to_string(&claimed_stars)?)
    .bind(&record)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(Act243Report {
        reply: Act243ReportEpisodeReply {
            activity_id: Some(activity_id),
            episode: Some(Act243EpisodeRecord {
                episode_id: Some(episode_id),
                episode_type: Some(episode.episode_type),
                is_finished: Some(true),
                best_score: Some(best_score),
                best_star: Some(best_star),
                best_time_ms: Some(best_time_ms),
                claimed_stars,
                record: Some(record),
            }),
            is_new_best: Some(is_new_best),
        },
        rewards,
        material_changes,
    })
}

fn merge_record(old: &str, incoming: &str) -> Result<(String, i32, i32), AppError> {
    let parse = |value: &str| {
        value
            .parse::<i32>()
            .map_err(|_| AppError::InvalidRequest)
            .and_then(|value| {
                let digits = [value / 100, value / 10 % 10, value % 10];
                digits
                    .iter()
                    .all(|digit| matches!(digit, 0 | 1))
                    .then_some(digits)
                    .ok_or(AppError::InvalidRequest)
            })
    };
    let old = if old.is_empty() { [0; 3] } else { parse(old)? };
    let incoming = parse(incoming)?;
    let digits = std::array::from_fn::<_, 3, _>(|index| old[index].max(incoming[index]));
    Ok((
        (digits[0] * 100 + digits[1] * 10 + digits[2]).to_string(),
        incoming.into_iter().sum(),
        digits.into_iter().sum(),
    ))
}
