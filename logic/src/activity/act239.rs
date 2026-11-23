use super::*;
use chrono::{NaiveDateTime, TimeZone, Utc};

const ACTIVITY_TYPE_ID: i32 = 239;

pub struct Act239Claim {
    pub reply: Act239BonusReply,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
    pub red_dot_id: i32,
    pub red_dot_info_ids: Vec<i32>,
}

pub async fn act239_info(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
) -> Result<GetAct239InfoReply, AppError> {
    act239_info_at(
        db,
        player_id,
        activity_id,
        common::time::ServerTime::now_ms(),
    )
    .await
}

pub async fn act239_bonus(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
    id: Option<i32>,
) -> Result<Act239Claim, AppError> {
    act239_bonus_at(
        db,
        player_id,
        activity_id,
        id,
        common::time::ServerTime::now_ms(),
    )
    .await
}

pub(crate) async fn act239_red_dot_entries(
    db: &SqlitePool,
    player_id: i64,
) -> Result<Vec<(i32, Vec<i32>)>, AppError> {
    act239_red_dot_entries_at(db, player_id, common::time::ServerTime::now_ms()).await
}

async fn act239_red_dot_entries_at(
    db: &SqlitePool,
    player_id: i64,
    now_ms: i64,
) -> Result<Vec<(i32, Vec<i32>)>, AppError> {
    let tables = config::configs::get();
    let mut activity_ids = tables
        .activity239
        .iter()
        .map(|row| row.activity_id)
        .collect::<Vec<_>>();
    activity_ids.sort_unstable();
    activity_ids.dedup();

    let mut entries = Vec::new();
    for activity_id in activity_ids {
        if !is_activity_active_at(activity_id, now_ms) {
            continue;
        }
        let activity = tables
            .activity
            .get(activity_id)
            .filter(|activity| activity.type_id == ACTIVITY_TYPE_ID)
            .ok_or(AppError::InvalidRequest)?;
        if activity.red_dot_id == 0 {
            continue;
        }
        let reply = act239_info_at(db, player_id, Some(activity_id), now_ms).await?;
        entries.push((
            activity.red_dot_id,
            reply
                .bonuss
                .into_iter()
                .filter(|bonus| bonus.status == Some(1))
                .filter_map(|bonus| bonus.id)
                .collect(),
        ));
    }
    Ok(entries)
}

async fn act239_info_at(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
    now_ms: i64,
) -> Result<GetAct239InfoReply, AppError> {
    let activity_id = resolve_activity_id(activity_id)?;
    let states =
        activity_state::get(db, player_id, activity_id, ActivityStateKind::Act239Bonus).await?;
    let mut bonuss = Vec::new();
    for row in config::configs::get()
        .activity239
        .iter()
        .filter(|row| row.activity_id == activity_id)
    {
        if parse_open_time_millis(&row.open_time).ok_or(AppError::InvalidRequest)? > now_ms {
            continue;
        }
        bonuss.push(Act239BonusNo {
            id: Some(row.id),
            status: Some(if states.get(&row.id).map(|state| state.0) == Some(2) {
                2
            } else {
                1
            }),
        });
    }
    bonuss.sort_unstable_by_key(|bonus| bonus.id.unwrap_or_default());

    Ok(GetAct239InfoReply {
        activity_id: Some(activity_id),
        bonuss,
    })
}

async fn act239_bonus_at(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
    id: Option<i32>,
    now_ms: i64,
) -> Result<Act239Claim, AppError> {
    let activity_id = resolve_activity_id(activity_id)?;
    let id = id.ok_or(AppError::InvalidRequest)?;
    let row = config::configs::get()
        .activity239
        .iter()
        .find(|row| row.activity_id == activity_id && row.id == id)
        .ok_or(AppError::InvalidRequest)?;
    if parse_open_time_millis(&row.open_time).ok_or(AppError::InvalidRequest)? > now_ms {
        return Err(AppError::InvalidRequest);
    }
    let parsed = reward::parse_strict(&row.bonus)?;
    let material_changes = parsed.material_changes();

    let mut tx = db.begin().await?;
    let claimed = activity_state::transition_in_transaction(
        &mut tx,
        player_id,
        activity_id,
        0,
        ActivityStateSet {
            kind: ActivityStateKind::Act239Bonus,
            entry_id: id,
            state: 2,
            progress: 0,
            ext: "",
        },
    )
    .await?;
    if !claimed {
        return Err(AppError::InvalidRequest);
    }
    let rewards = reward::apply_in_transaction(&mut tx, db, player_id, parsed).await?;
    tx.commit().await?;

    let info = act239_info_at(db, player_id, Some(activity_id), now_ms).await?;
    let red_dot_info_ids = info
        .bonuss
        .iter()
        .filter(|bonus| bonus.status == Some(1))
        .filter_map(|bonus| bonus.id)
        .collect();
    let red_dot_id = config::configs::get()
        .activity
        .get(activity_id)
        .ok_or(AppError::InvalidRequest)?
        .red_dot_id;
    Ok(Act239Claim {
        reply: Act239BonusReply {
            activity_id: info.activity_id,
            bonuss: info.bonuss,
        },
        rewards,
        material_changes,
        red_dot_id,
        red_dot_info_ids,
    })
}

fn resolve_activity_id(activity_id: Option<i32>) -> Result<i32, AppError> {
    let activity_id = activity_id.ok_or(AppError::InvalidRequest)?;
    let tables = config::configs::get();
    tables
        .activity
        .get(activity_id)
        .filter(|activity| activity.type_id == ACTIVITY_TYPE_ID)
        .filter(|_| {
            tables
                .activity239
                .iter()
                .any(|row| row.activity_id == activity_id)
        })
        .map(|_| activity_id)
        .ok_or(AppError::InvalidRequest)
}

fn parse_open_time_millis(value: &str) -> Option<i64> {
    NaiveDateTime::parse_from_str(value.trim(), "%Y-%m-%d %H:%M:%S")
        .ok()
        .and_then(|time| Utc.from_local_datetime(&time).single())
        .map(|time| time.timestamp_millis() - common::time::ServerTime::server_utc_offset_ms())
}

fn is_activity_active_at(activity_id: i32, now_ms: i64) -> bool {
    u64::try_from(now_ms).ok().is_some_and(|now_ms| {
        super::schedule::get(activity_id)
            .is_some_and(|schedule| schedule.start_time <= now_ms && now_ms <= schedule.end_time)
    })
}

#[cfg(test)]
mod tests;
