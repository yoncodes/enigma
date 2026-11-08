use super::*;
use common::time::ServerTime;
use sonettobuf::{Act243UnlockRacerReply, Act243UpgradeGiftReply};

pub struct Act243Cost<T> {
    pub reply: T,
    pub consumed: reward::ConsumedRewards,
}

pub async fn upgrade_gift(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
    gift_point: i32,
) -> Result<Act243Cost<Act243UpgradeGiftReply>, AppError> {
    validate_activity(activity_id)?;
    let mut tx = db.begin().await?;
    let current = sqlx::query_scalar::<_, i32>(
        "SELECT level FROM user_activity243_gifts
         WHERE user_id = ? AND activity_id = ? AND gift_point = ?",
    )
    .bind(player_id)
    .bind(activity_id)
    .bind(gift_point)
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or_default();
    let next = config::configs::get()
        .racing_gift
        .iter()
        .find(|row| {
            row.activity_id == activity_id
                && row.gift_point == gift_point
                && row.level == current + 1
        })
        .ok_or(AppError::InvalidRequest)?;
    if current == 0 && next.order > 1 {
        let previous = config::configs::get()
            .racing_gift
            .iter()
            .filter(|row| {
                row.activity_id == activity_id
                    && row.group == next.group
                    && row.order == next.order - 1
            })
            .max_by_key(|row| row.level)
            .ok_or(AppError::InvalidRequest)?;
        let previous_level = sqlx::query_scalar::<_, i32>(
            "SELECT level FROM user_activity243_gifts
             WHERE user_id = ? AND activity_id = ? AND gift_point = ?",
        )
        .bind(player_id)
        .bind(activity_id)
        .bind(previous.gift_point)
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or_default();
        if previous_level != previous.level {
            return Err(AppError::InvalidRequest);
        }
    }

    let mut costs = reward::RewardSet::default();
    costs
        .currencies
        .push((currency_id(activity_id)?, next.cost));
    let now = ServerTime::now_ms();
    let consumed = reward::RewardManager::new(player_id)
        .consume(&mut tx, &costs)
        .await?;
    sqlx::query(
        "INSERT INTO user_activity243_gifts
            (user_id, activity_id, gift_point, level, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(user_id, activity_id, gift_point) DO UPDATE SET
            level = excluded.level,
            updated_at = excluded.updated_at",
    )
    .bind(player_id)
    .bind(activity_id)
    .bind(gift_point)
    .bind(next.level)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(Act243Cost {
        reply: Act243UpgradeGiftReply {
            activity_id: Some(activity_id),
            gift: Some(Act243GiftRecord {
                gift_point: Some(gift_point),
                level: Some(next.level),
            }),
        },
        consumed,
    })
}

pub async fn unlock_racer(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
    racer_id: i32,
) -> Result<Act243Cost<Act243UnlockRacerReply>, AppError> {
    validate_activity(activity_id)?;
    if !has_racing_progression(activity_id) {
        return Err(AppError::InvalidRequest);
    }
    let racer = config::configs::get()
        .racing_racer
        .get(racer_id)
        .filter(|racer| racer.display == 1 && racer.cost > 0)
        .ok_or(AppError::InvalidRequest)?;
    let now = ServerTime::now_ms();
    let mut costs = reward::RewardSet::default();
    costs
        .currencies
        .push((currency_id(activity_id)?, racer.cost));
    let mut tx = db.begin().await?;
    if sqlx::query_scalar::<_, i32>(
        "SELECT 1 FROM user_activity243_racers
         WHERE user_id = ? AND activity_id = ? AND racer_id = ?",
    )
    .bind(player_id)
    .bind(activity_id)
    .bind(racer_id)
    .fetch_optional(&mut *tx)
    .await?
    .is_some()
    {
        return Err(AppError::InvalidRequest);
    }
    let consumed = reward::RewardManager::new(player_id)
        .consume(&mut tx, &costs)
        .await?;
    sqlx::query(
        "INSERT INTO user_activity243_racers
            (user_id, activity_id, racer_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(player_id)
    .bind(activity_id)
    .bind(racer_id)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(Act243Cost {
        reply: Act243UnlockRacerReply {
            activity_id: Some(activity_id),
            racer_id: Some(racer_id),
        },
        consumed,
    })
}
