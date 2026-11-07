use crate::error::AppError;
use database::{
    db::game::{tasks as task_db, turnback},
    models::game::turnback::TurnbackDropKind,
};
use sonettobuf::{
    DropInfo, GetTurnbackInfoReply, TurnbackFirstShowReply, TurnbackInfo, TurnbackSignInInfo,
};
use sqlx::SqlitePool;
use std::collections::BTreeMap;

pub(super) async fn sync_state(
    db: &SqlitePool,
    player_id: i64,
    tables: &config::GameDB,
) -> Result<(), AppError> {
    let Some(state) = turnback::get_active_state(db, player_id, tables).await? else {
        return Ok(());
    };
    task_db::ensure_turnback_tasks(db, player_id, state.turnback_id, tables).await?;
    turnback::ensure_sign_ins(db, player_id, state.turnback_id, tables).await?;
    turnback::ensure_drops(db, player_id, state.turnback_id, tables).await?;
    Ok(())
}

pub(super) async fn turnback_info(
    db: &SqlitePool,
    player_id: i64,
    tables: &config::GameDB,
) -> Result<GetTurnbackInfoReply, AppError> {
    let Some(state) = turnback::get_active_state(db, player_id, tables).await? else {
        return Ok(GetTurnbackInfoReply { info: None });
    };
    let tasks = task_db::list_turnback(db, player_id, state.turnback_id)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();
    let sign_in_infos = turnback::list_sign_ins(db, player_id, state.turnback_id)
        .await?
        .into_iter()
        .map(|row| TurnbackSignInInfo {
            id: Some(row.day),
            state: Some(row.state),
        })
        .collect();
    let drop_state = turnback::list_drops(db, player_id, state.turnback_id)
        .await?
        .into_iter()
        .map(|row| (row.drop_id, row.current_num))
        .collect::<BTreeMap<_, _>>();
    let drop_infos = tables
        .turnback_drop
        .iter()
        .filter(|row| row.r#type == TurnbackDropKind::Progress.id())
        .map(|row| DropInfo {
            r#type: Some(row.id),
            total_num: Some(row.level),
            current_num: Some(*drop_state.get(&row.id).unwrap_or(&0)),
        })
        .collect();

    Ok(GetTurnbackInfoReply {
        info: Some(TurnbackInfo {
            id: Some(state.turnback_id),
            tasks,
            bonus_point: Some(state.bonus_point),
            first_show: Some(state.first_show),
            has_get_task_bonus: state.claimed_task_bonus_ids()?,
            sign_in_day: Some(state.sign_in_day),
            sign_in_infos,
            once_bonus: Some(state.once_bonus),
            end_time: Some(state.end_time),
            start_time: Some(state.start_time),
            remain_addition_count: Some(state.remain_addition_count),
            leave_time: Some(state.leave_time),
            month_card_added_buy_count: Some(state.month_card_added_buy_count),
            version: Some(state.version),
            buy_double_bonus: Some(state.buy_double_bonus),
            drop_infos,
            get_daily_bonus: Some(state.get_daily_bonus),
            common_drop_infos: Vec::new(),
            resource_return: None,
        }),
    })
}

pub(super) async fn turnback_first_show(
    db: &SqlitePool,
    player_id: i64,
    turnback_id: i32,
) -> Result<TurnbackFirstShowReply, AppError> {
    let first_show = turnback::mark_first_show(db, player_id, turnback_id).await?;
    Ok(TurnbackFirstShowReply {
        id: first_show.then_some(turnback_id),
    })
}
