use super::*;
use chrono::{NaiveDateTime, TimeZone, Utc};

mod tasks;
pub use tasks::Act186TaskClaim;
pub(super) use tasks::{finish_all, sync as sync_tasks, sync_login};

#[repr(i32)]
enum Act186ConstId {
    MilestoneCurrency = 1,
}

pub async fn act186_info(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
) -> Result<GetAct186InfoReply, AppError> {
    let tables = config::configs::get();
    let activity_id = activity_id
        .or_else(|| {
            tables
                .actvity186_stage
                .iter()
                .map(|row| row.activity_id)
                .max()
        })
        .ok_or(AppError::InvalidRequest)?;
    let task_states =
        activity_state::get(db, player_id, activity_id, ActivityStateKind::Act186Task).await?;
    let like_states =
        activity_state::get(db, player_id, activity_id, ActivityStateKind::Act186Like).await?;
    let now = common::time::ServerTime::now_ms();
    let activity_end = tables
        .actvity186_stage
        .iter()
        .filter(|row| row.activity_id == activity_id)
        .map(|row| parse_time_millis(&row.end_time))
        .max()
        .unwrap_or_default();
    let milestone_currency_id = tables
        .activity186_const
        .get(Act186ConstId::MilestoneCurrency as i32)
        .and_then(|row| row.value.parse::<i32>().ok())
        .ok_or(AppError::InvalidRequest)?;
    let milestone_progress =
        database::db::game::currencies::get_currency(db, player_id, milestone_currency_id)
            .await?
            .map(|currency| currency.quantity)
            .unwrap_or_default();

    let mut task_infos = tables
        .actvity186_task
        .iter()
        .filter(|row| row.activity_id == activity_id && row.is_online != 0)
        .map(|row| {
            let (state, progress, _) =
                task_states
                    .get(&row.id)
                    .cloned()
                    .unwrap_or((0, 0, String::new()));

            Act186TaskInfo {
                task_id: Some(row.id),
                progress: Some(progress),
                expire_time: Some(task_expire_time(row.loop_type, activity_end, now)),
                has_get_bonus: Some(state != 0),
            }
        })
        .collect::<Vec<_>>();
    task_infos.sort_by_key(|task| task.task_id.unwrap_or_default());

    let mut like_infos = tables
        .actvity186_like
        .iter()
        .filter(|row| row.activity_id == activity_id)
        .map(|row| Act186LikeInfo {
            like_type: Some(row.r#type),
            value: Some(
                like_states
                    .get(&row.r#type)
                    .map(|(state, _, _)| *state)
                    .unwrap_or_default(),
            ),
        })
        .collect::<Vec<_>>();
    like_infos.sort_by_key(|like| like.like_type.unwrap_or_default());

    let mut game_infos = tables
        .actvity186_mini_game
        .iter()
        .filter(|row| row.activity_id == activity_id)
        .map(|row| Act186GameInfo {
            game_id: Some(row.id),
            game_type_id: Some(first_weighted_id(&row.game_type2_prob).unwrap_or_default()),
            expire_time: Some(0),
            b_type_game_info: None,
        })
        .collect::<Vec<_>>();
    game_infos.sort_by_key(|game| game.game_id.unwrap_or_default());

    Ok(GetAct186InfoReply {
        activity_id: Some(activity_id),
        info: Some(Act186Info {
            current_stage: Some(current_stage(activity_id)),
            get_milestone_progress: Some(i64::from(milestone_progress)),
            get_daily_collection: Some(false),
            get_onece_bonus: Some(false),
        }),
        task_infos,
        like_infos,
        game_infos,
    })
}

pub async fn get_act186_sp_bonus_info(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
    act186_activity_id: Option<i32>,
) -> Result<GetAct186SpBonusInfoReply, AppError> {
    let activity_id = activity_id.ok_or(AppError::InvalidRequest)?;
    let act186_activity_id = act186_activity_id.ok_or(AppError::InvalidRequest)?;
    let states =
        activity_state::get(db, player_id, activity_id, ActivityStateKind::Act186SpBonus).await?;

    Ok(GetAct186SpBonusInfoReply {
        activity_id: Some(activity_id),
        act186_activity_id: Some(act186_activity_id),
        sp_bonus_stage: Some(
            states
                .get(&act186_activity_id)
                .map(|(state, _, _)| *state)
                .unwrap_or(0),
        ),
    })
}

fn current_stage(activity_id: i32) -> i32 {
    let now = common::time::ServerTime::now_ms();
    let mut latest_past = None;

    for row in config::configs::get()
        .actvity186_stage
        .iter()
        .filter(|row| row.activity_id == activity_id)
    {
        let start = parse_time_millis(&row.start_time);
        let end = parse_time_millis(&row.end_time);

        if start <= now && now <= end {
            return row.stage_id;
        }
        if end <= now {
            latest_past = Some(row.stage_id);
        }
    }

    latest_past.unwrap_or(1)
}

fn parse_time_millis(value: &str) -> i64 {
    if value.trim().is_empty() {
        return 0;
    }

    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%-m-%d %H:%M:%S"))
        .map(|dt| {
            Utc.from_utc_datetime(&dt).timestamp_millis()
                - common::time::ServerTime::server_utc_offset_ms()
        })
        .unwrap_or(0)
}

fn task_expire_time(loop_type: i32, activity_end: i64, now: i64) -> i64 {
    use database::db::game::tasks::TaskLoopType;

    match TaskLoopType::from_id(loop_type) {
        Some(TaskLoopType::Daily) => {
            i64::from(common::time::ServerTime::next_daily_refresh_sec(now)) * 1_000
        }
        Some(TaskLoopType::Weekly) => {
            i64::from(common::time::ServerTime::next_weekly_refresh_sec(now)) * 1_000
        }
        _ => activity_end,
    }
}

fn first_weighted_id(value: &str) -> Option<i32> {
    value
        .split('|')
        .find_map(|part| part.split('#').next()?.parse().ok())
}

pub async fn accept_act186_sp_bonus(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
    act186_activity_id: Option<i32>,
) -> Result<AcceptAct186SpBonusReply, AppError> {
    let activity_id = activity_id.ok_or(AppError::InvalidRequest)?;
    let act186_activity_id = act186_activity_id.ok_or(AppError::InvalidRequest)?;
    activity_state::set(
        db,
        player_id,
        activity_id,
        ActivityStateSet {
            kind: ActivityStateKind::Act186SpBonus,
            entry_id: act186_activity_id,
            state: 2,
            progress: 0,
            ext: "",
        },
    )
    .await?;

    Ok(AcceptAct186SpBonusReply {
        activity_id: Some(activity_id),
        act186_activity_id: Some(act186_activity_id),
    })
}

#[cfg(test)]
mod tests;
