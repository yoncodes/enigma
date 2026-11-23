use super::*;
use database::db::game::activity236;
use sonettobuf::{Act236GetAutoGainRewardReply, Act236Info, GetAct236InfoReply};
use std::collections::HashSet;

const ACTIVITY_TYPE_ID: i32 = 236;

pub struct Act236RewardClaim {
    pub reply: Act236GetAutoGainRewardReply,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
    pub red_dot_id: i32,
    pub red_dot_value: i32,
}

pub struct Act236ChargeUpdate {
    pub info: Act236Info,
    pub red_dot_id: i32,
    pub claimable_reward_ids: Vec<i32>,
}

pub(crate) fn active_act236_activity_id_at(now_ms: i64) -> Option<i32> {
    let now_ms = u64::try_from(now_ms).ok()?;
    let tables = config::configs::get();

    tables
        .activity236_control
        .iter()
        .filter_map(|control| {
            let activity = tables.activity.get(control.id)?;
            let schedule = super::schedule::get(control.id)?;
            (activity.type_id == ACTIVITY_TYPE_ID
                && (activity.open_id == 0
                    || tables
                        .open
                        .get(activity.open_id)
                        .is_some_and(|open| open.is_online != 0))
                && (schedule.start_time..=schedule.end_time).contains(&now_ms))
            .then_some(control.id)
        })
        .max()
}

pub(crate) fn act236_charge_update(
    activity_id: i32,
    state: activity236::Activity236State,
) -> Result<Act236ChargeUpdate, AppError> {
    let tables = config::configs::get();
    let activity = tables
        .activity
        .get(activity_id)
        .filter(|activity| activity.type_id == ACTIVITY_TYPE_ID)
        .ok_or(AppError::InvalidRequest)?;
    if activity.red_dot_id <= 0 {
        return Err(AppError::InvalidRequest);
    }

    let claimable_reward_ids = tables
        .activity236
        .iter()
        .filter(|row| {
            row.activity_id == activity_id
                && row.cost <= state.score
                && !state.gain_reward_ids.contains(&row.id)
        })
        .map(|row| row.id)
        .collect();

    Ok(Act236ChargeUpdate {
        info: Act236Info {
            activity_id: Some(activity_id),
            score: Some(state.score),
            gain_reward_ids: state.gain_reward_ids,
        },
        red_dot_id: activity.red_dot_id,
        claimable_reward_ids,
    })
}

pub async fn act236_info(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
) -> Result<GetAct236InfoReply, AppError> {
    let activity_id = resolve_activity_id(activity_id)?;
    let state = activity236::get_or_create_state(db, player_id, activity_id).await?;

    Ok(GetAct236InfoReply {
        info: Some(Act236Info {
            activity_id: Some(activity_id),
            score: Some(state.score),
            gain_reward_ids: state.gain_reward_ids,
        }),
    })
}

pub async fn act236_get_auto_gain_reward(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
    reward_ids: Vec<i32>,
) -> Result<Act236RewardClaim, AppError> {
    let activity_id = resolve_activity_id(activity_id)?;
    if reward_ids.is_empty()
        || reward_ids.iter().copied().collect::<HashSet<_>>().len() != reward_ids.len()
    {
        return Err(AppError::InvalidRequest);
    }

    let tables = config::configs::get();
    let activity = tables
        .activity
        .get(activity_id)
        .ok_or(AppError::InvalidRequest)?;
    if activity.red_dot_id <= 0 {
        return Err(AppError::InvalidRequest);
    }

    let mut parsed = reward::RewardSet::default();
    for reward_id in &reward_ids {
        let row = tables
            .activity236
            .get(*reward_id)
            .filter(|row| row.activity_id == activity_id)
            .ok_or(AppError::InvalidRequest)?;
        parsed.extend(reward::parse_strict(&row.reward)?);
    }
    let material_changes = parsed.material_changes();

    let state = activity236::get_or_create_state(db, player_id, activity_id).await?;
    for reward_id in &reward_ids {
        let row = tables
            .activity236
            .get(*reward_id)
            .ok_or(AppError::InvalidRequest)?;
        if row.cost > state.score || state.gain_reward_ids.contains(reward_id) {
            return Err(AppError::InvalidRequest);
        }
    }

    let mut gained = state.gain_reward_ids.clone();
    gained.extend(reward_ids.iter().copied());
    let mut tx = db.begin().await?;
    if !activity236::claim_rewards_in_transaction(&mut tx, player_id, activity_id, &state, &gained)
        .await?
    {
        return Err(AppError::InvalidRequest);
    }
    let rewards = reward::apply_in_transaction(&mut tx, db, player_id, parsed).await?;
    tx.commit().await?;

    let red_dot_value = i32::from(tables.activity236.iter().any(|row| {
        row.activity_id == activity_id && row.cost <= state.score && !gained.contains(&row.id)
    }));

    Ok(Act236RewardClaim {
        reply: Act236GetAutoGainRewardReply {
            activity_id: Some(activity_id),
            gain_reward_ids: reward_ids,
        },
        rewards,
        material_changes,
        red_dot_id: activity.red_dot_id,
        red_dot_value,
    })
}

fn resolve_activity_id(activity_id: Option<i32>) -> Result<i32, AppError> {
    let tables = config::configs::get();
    let activity_id = activity_id.ok_or(AppError::InvalidRequest)?;

    tables
        .activity
        .get(activity_id)
        .filter(|activity| activity.type_id == ACTIVITY_TYPE_ID)
        .map(|_| activity_id)
        .ok_or(AppError::InvalidRequest)
}

#[cfg(test)]
mod tests;
