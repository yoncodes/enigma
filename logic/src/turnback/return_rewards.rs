use super::claims;
use crate::{error::AppError, reward, reward::RewardedReply};
use database::{
    db::game::{currencies, tasks as task_db, turnback},
    models::game::{tasks::UserTask, turnback::TurnbackState},
};
use sonettobuf::{
    AcceptAllTurnbackReturnRewardReply, TurnbackResourceReturnInfo, TurnbackReturnRewardInfo,
    TurnbackReturnRewardReply,
};
use sqlx::SqlitePool;
use std::collections::{BTreeSet, HashMap, HashSet};

const DAY_SECONDS: i32 = 24 * 60 * 60;

#[derive(Clone, Copy)]
enum RewardState {
    NotFinish = 0,
    CanGet = 1,
    HasGet = 2,
}

#[derive(Default)]
struct Rotation {
    active: i32,
    left: Vec<i32>,
    last: i32,
    refresh_day: i64,
}

pub(super) async fn info(
    db: &SqlitePool,
    player_id: i64,
    state: &TurnbackState,
    tasks: &[UserTask],
    tables: &config::GameDB,
) -> Result<Option<TurnbackResourceReturnInfo>, AppError> {
    let Some((range_id, currency_id)) = context(state, tables)? else {
        return Ok(None);
    };
    let vitality = currencies::get_currency(db, player_id, currency_id)
        .await?
        .map(|currency| currency.quantity)
        .unwrap_or_default();
    let claimed = state
        .claimed_return_reward_ids()?
        .into_iter()
        .collect::<HashSet<_>>();
    let open = range_id != 0 && event_open(state);
    let rewards = open
        .then(|| {
            tables
                .turnback_return_reward
                .iter()
                .filter(|row| {
                    row.turnback_id == state.turnback_id
                        && row.loss_days_range == range_id
                        && row.is_online != 0
                })
                .map(|row| TurnbackReturnRewardInfo {
                    reward_id: Some(row.id),
                    need_vitality: Some(row.need_vitality),
                    state: Some(if claimed.contains(&row.id) {
                        RewardState::HasGet as i32
                    } else if vitality >= row.need_vitality {
                        RewardState::CanGet as i32
                    } else {
                        RewardState::NotFinish as i32
                    }),
                })
                .collect()
        })
        .unwrap_or_default();
    let rotation = if open {
        rotation(state)?
    } else {
        Rotation::default()
    };
    let mut return_task_ids = tables
        .turnback_task
        .iter()
        .filter(|row| {
            row.turnback_id == state.turnback_id
                && row.turnback_task_type == 1
                && row.is_online != 0
        })
        .map(|row| row.id)
        .collect::<HashSet<_>>();
    if rotation.active != 0 {
        return_task_ids.insert(rotation.active);
    }
    let resource_return_tasks = tasks
        .iter()
        .filter(|task| return_task_ids.contains(&task.task_id))
        .cloned()
        .map(Into::into)
        .collect();

    Ok(Some(TurnbackResourceReturnInfo {
        open: Some(open),
        loss_days_range_id: Some(range_id),
        vitality: Some(vitality),
        rewards,
        resource_return_tasks,
        left_task_ids: rotation.left,
        last_finish_random_task_id: (rotation.last != 0).then_some(rotation.last),
    }))
}

pub(super) async fn sync_tasks(
    db: &SqlitePool,
    player_id: i64,
    state: &TurnbackState,
    tables: &config::GameDB,
) -> Result<TurnbackState, AppError> {
    if context(state, tables)?.is_none() || !event_open(state) {
        return Ok(state.clone());
    }
    let tasks = task_db::list_turnback(db, player_id, state.turnback_id).await?;
    rotate(db, player_id, state, &tasks, tables).await
}

pub(super) async fn validate_tasks(
    db: &SqlitePool,
    player_id: i64,
    task_ids: &[i32],
    tables: &config::GameDB,
) -> Result<(), AppError> {
    let state = turnback::get_active_state(db, player_id, tables)
        .await?
        .ok_or(AppError::InvalidRequest)?;
    let state = sync_tasks(db, player_id, &state, tables).await?;
    for task_id in task_ids {
        let task = tables
            .turnback_task
            .get(*task_id)
            .filter(|task| task.turnback_id == state.turnback_id && task.is_online != 0)
            .ok_or(AppError::InvalidRequest)?;
        if matches!(task.turnback_task_type, 1 | 2)
            && (!event_open(&state)
                || context(&state, tables)?.is_none()
                || task.turnback_task_type == 2 && state.return_random_task_id != *task_id)
        {
            return Err(AppError::InvalidRequest);
        }
    }
    Ok(())
}

pub(super) async fn claim_one(
    db: &SqlitePool,
    player_id: i64,
    turnback_id: i32,
    reward_id: i32,
    tables: &config::GameDB,
) -> Result<RewardedReply<TurnbackReturnRewardReply>, AppError> {
    claim(
        db,
        player_id,
        turnback_id,
        Some(reward_id),
        tables,
        |claimed| TurnbackReturnRewardReply {
            id: Some(turnback_id),
            reward_id: Some(reward_id),
            has_get_return_reward_ids: claimed,
        },
    )
    .await
}

pub(super) async fn claim_all(
    db: &SqlitePool,
    player_id: i64,
    turnback_id: i32,
    tables: &config::GameDB,
) -> Result<RewardedReply<AcceptAllTurnbackReturnRewardReply>, AppError> {
    claim(db, player_id, turnback_id, None, tables, |claimed| {
        AcceptAllTurnbackReturnRewardReply {
            id: Some(turnback_id),
            has_get_return_reward_ids: claimed,
        }
    })
    .await
}

async fn claim<T>(
    db: &SqlitePool,
    player_id: i64,
    turnback_id: i32,
    reward_id: Option<i32>,
    tables: &config::GameDB,
    reply: impl FnOnce(Vec<i32>) -> T,
) -> Result<RewardedReply<T>, AppError> {
    let mut tx = db.begin().await?;
    let state = turnback::get_active_state_in_transaction(&mut tx, player_id, tables)
        .await?
        .filter(|state| state.turnback_id == turnback_id)
        .ok_or(AppError::InvalidRequest)?;
    let (range_id, currency_id) = context(&state, tables)?.ok_or(AppError::InvalidRequest)?;
    if range_id == 0 || !event_open(&state) {
        return Err(AppError::InvalidRequest);
    }
    let vitality = currencies::quantity_in_transaction(&mut tx, player_id, currency_id).await?;
    let rows = tables
        .turnback_return_reward
        .iter()
        .filter(|row| {
            row.turnback_id == turnback_id
                && row.loss_days_range == range_id
                && row.is_online != 0
                && row.need_vitality <= vitality
        })
        .collect::<Vec<_>>();
    let mut claimed = state
        .claimed_return_reward_ids()?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let selected = match reward_id {
        Some(id) => rows
            .into_iter()
            .filter(|row| row.id == id && !claimed.contains(&id))
            .collect::<Vec<_>>(),
        None => rows
            .into_iter()
            .filter(|row| !claimed.contains(&row.id))
            .collect::<Vec<_>>(),
    };
    if reward_id.is_some() && selected.is_empty() {
        return Err(AppError::InvalidRequest);
    }

    let changed = !selected.is_empty();
    let mut rewards = reward::RewardSet::default();
    for row in selected {
        claimed.insert(row.id);
        rewards.extend(reward::parse_strict(&row.bonus)?);
    }
    let claimed = claimed.into_iter().collect::<Vec<_>>();
    if changed {
        turnback::save_claimed_return_rewards_in_transaction(
            &mut tx,
            player_id,
            turnback_id,
            &state.has_get_return_reward_ids,
            &claimed,
        )
        .await?
        .ok_or(AppError::InvalidRequest)?;
    }
    claims::rewarded_reply_in_transaction(tx, db, player_id, reply(claimed), rewards).await
}

fn context(state: &TurnbackState, tables: &config::GameDB) -> Result<Option<(i32, i32)>, AppError> {
    let config = tables
        .turnback
        .get(state.turnback_id)
        .ok_or(AppError::InvalidRequest)?;
    if config.resource_return_condition.is_empty() {
        return Ok(None);
    }
    let threshold = config
        .resource_return_condition
        .strip_prefix("LossDays>=")
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or(AppError::InvalidRequest)?;
    let loss_days = state
        .start_time
        .saturating_sub(state.leave_time)
        .div_euclid(DAY_SECONDS);
    let range_id =
        if state.leave_time > 0 && state.start_time > state.leave_time && loss_days >= threshold {
            range_id(loss_days, tables)?
        } else {
            0
        };
    Ok(Some((
        range_id,
        vitality_currency_id(&config.resource_return_vitality)?,
    )))
}

pub(super) fn range_id(loss_days: i32, tables: &config::GameDB) -> Result<i32, AppError> {
    let mut fallback = None;
    for row in tables.turnback_return_range.iter() {
        let (start, end) = row
            .loss_days
            .split_once('|')
            .and_then(|(start, end)| Some((start.parse().ok()?, end.parse().ok()?)))
            .ok_or(AppError::InvalidRequest)?;
        if loss_days >= start && loss_days < end {
            return Ok(row.range_id);
        }
        if loss_days >= start && fallback.is_none_or(|(max, _)| end > max) {
            fallback = Some((end, row.range_id));
        }
    }
    fallback.map(|(_, id)| id).ok_or(AppError::InvalidRequest)
}

async fn rotate(
    db: &SqlitePool,
    player_id: i64,
    state: &TurnbackState,
    tasks: &[UserTask],
    tables: &config::GameDB,
) -> Result<TurnbackState, AppError> {
    let mut candidates = tables
        .turnback_task
        .iter()
        .filter(|row| {
            row.turnback_id == state.turnback_id
                && row.turnback_task_type == 2
                && row.is_online != 0
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|row| (row.random_priority, row.sort_id, row.id));
    let candidate_ids = candidates.iter().map(|row| row.id).collect::<HashSet<_>>();
    let task_states = tasks
        .iter()
        .map(|task| (task.task_id, task))
        .collect::<HashMap<_, _>>();
    let mut rotation = rotation(state)?;
    let today = common::time::ServerTime::server_day(common::time::ServerTime::now_ms());
    if rotation.active == 0 && rotation.left.is_empty() && rotation.last == 0 {
        let mut ids = candidates.into_iter().map(|row| row.id);
        rotation.active = ids.next().unwrap_or_default();
        rotation.left = ids.collect();
        rotation.refresh_day = today;
    }
    if (rotation.active != 0 && !candidate_ids.contains(&rotation.active))
        || (rotation.active != 0 && !task_states.contains_key(&rotation.active))
        || rotation.left.iter().any(|id| !candidate_ids.contains(id))
        || rotation.left.iter().copied().collect::<HashSet<_>>().len() != rotation.left.len()
        || rotation.left.contains(&rotation.active)
    {
        return Err(AppError::InvalidRequest);
    }
    let day_changed = rotation.refresh_day != 0 && today > rotation.refresh_day;
    if day_changed {
        task_db::reset_turnback_return_daily_tasks(db, player_id, state.turnback_id, tables)
            .await?;
    }
    if rotation.active != 0
        && task_states
            .get(&rotation.active)
            .is_some_and(|task| task.finish_count > 0)
    {
        rotation.last = rotation.active;
        rotation.active = 0;
    }
    if rotation.active == 0 && !rotation.left.is_empty() && day_changed {
        rotation.active = rotation.left.remove(0);
    }
    if day_changed {
        rotation.refresh_day = today;
    }

    let changed = rotation.active != state.return_random_task_id
        || rotation.left != state.return_left_task_ids()?
        || rotation.last != state.return_last_random_task_id
        || rotation.refresh_day != state.return_random_refresh_day;
    if changed
        && !turnback::save_return_task_rotation(
            db,
            player_id,
            state.turnback_id,
            state.return_random_task_id,
            &state.return_left_task_ids,
            state.return_last_random_task_id,
            state.return_random_refresh_day,
            rotation.active,
            &rotation.left,
            rotation.last,
            rotation.refresh_day,
        )
        .await?
    {
        return turnback::get_active_state(db, player_id, tables)
            .await?
            .filter(|current| current.turnback_id == state.turnback_id)
            .ok_or(AppError::InvalidRequest);
    }
    if !changed {
        return Ok(state.clone());
    }
    let mut current = state.clone();
    current.return_random_task_id = rotation.active;
    current.return_left_task_ids = serde_json::to_string(&rotation.left)?;
    current.return_last_random_task_id = rotation.last;
    current.return_random_refresh_day = rotation.refresh_day;
    Ok(current)
}

fn rotation(state: &TurnbackState) -> Result<Rotation, AppError> {
    Ok(Rotation {
        active: state.return_random_task_id,
        left: state.return_left_task_ids()?,
        last: state.return_last_random_task_id,
        refresh_day: state.return_random_refresh_day,
    })
}

fn event_open(state: &TurnbackState) -> bool {
    let now = common::time::ServerTime::now_sec_i32();
    state.start_time > 0 && state.end_time > 0 && now >= state.start_time && now <= state.end_time
}

fn vitality_currency_id(value: &str) -> Result<i32, AppError> {
    let (kind, id) = value.split_once('#').ok_or(AppError::InvalidRequest)?;
    let kind = kind.parse::<u32>().map_err(|_| AppError::InvalidRequest)?;
    let id = id.parse::<i32>().map_err(|_| AppError::InvalidRequest)?;
    (kind == reward::RewardMaterialType::Currency.id() && id > 0)
        .then_some(id)
        .ok_or(AppError::InvalidRequest)
}
