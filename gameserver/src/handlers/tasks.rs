use crate::{
    error::AppError,
    logic::{bp, task as tasks},
    net::{context::ConnectionContext, packet::ClientPacket},
    types::material_get_approach::MaterialGetApproach,
    util::{push, task_events},
};
use logic::task::{TaskEvent, TaskType};
use prost::Message;
use sonettobuf::{
    Act233BpScoreUpdatePush, CmdId, FinishAllTaskRequest, FinishReadTaskRequest, FinishTaskRequest,
    GetTaskActivityBonusRequest, GetTaskInfoRequest, RefreshOnlineTaskRequest, UpdateTaskPush,
};

pub async fn on_get_task_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = GetTaskInfoRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let reply = ctx.player_mut()?.tasks.get_info(db, msg.type_ids).await?;

    ctx.send_reply(CmdId::GetTaskInfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_finish_task(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = FinishTaskRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let player_id = ctx.player()?.id;
    let claim = ctx.player_mut()?.tasks.finish(db, msg.id).await?;
    let finished_tasks = claim.task_info.clone();
    let act233_bp_scores = claim.act233_bp_scores.clone();
    let finished_task_ids = claim
        .task_info
        .iter()
        .map(|task| task.id)
        .collect::<Vec<_>>();

    let reward_approach = task_reward_approach(&finished_tasks);
    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(reward_approach),
    )
    .await?;
    send_bp_task_red_dot_update(ctx, &finished_tasks).await?;
    notify_task_finish_events(ctx, player_id, &finished_task_ids).await?;
    send_task_update(ctx, claim.task_info, claim.activity_info).await?;
    send_act233_bp_score_updates(ctx, &act233_bp_scores).await?;

    ctx.send_reply(CmdId::FinishTaskCmd, claim.reply, 0, req.up_tag)
        .await
}

pub async fn on_finish_all_task(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = FinishAllTaskRequest::decode(&req.data[..])?;
    let type_id = msg.type_id.ok_or(AppError::InvalidRequest)?;
    let db = ctx.state.db;
    let player_id = ctx.player()?.id;
    let claim = ctx
        .player_mut()?
        .tasks
        .finish_all(db, type_id, msg.min_type_id, msg.task_ids, msg.activity_id)
        .await?;
    let finished_tasks = claim.task_info.clone();
    let act233_bp_scores = claim.act233_bp_scores.clone();
    let finished_task_ids = claim
        .task_info
        .iter()
        .map(|task| task.id)
        .collect::<Vec<_>>();

    let reward_approach = task_reward_approach(&finished_tasks);
    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(reward_approach),
    )
    .await?;
    send_bp_task_red_dot_update(ctx, &finished_tasks).await?;
    notify_task_finish_events(ctx, player_id, &finished_task_ids).await?;
    send_task_update(ctx, claim.task_info, claim.activity_info).await?;
    send_act233_bp_score_updates(ctx, &act233_bp_scores).await?;

    ctx.send_reply(CmdId::FinishAllTaskCmd, claim.reply, 0, req.up_tag)
        .await
}

pub async fn on_get_task_activity_bonus(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = GetTaskActivityBonusRequest::decode(&req.data[..])?;
    let type_id = msg.type_id.ok_or(AppError::InvalidRequest)?;
    let define_id = msg.define_id.ok_or(AppError::InvalidRequest)?;
    let db = ctx.state.db;
    let player_id = ctx.player()?.id;
    let claim = ctx
        .player_mut()?
        .tasks
        .get_activity_bonus(db, type_id, define_id)
        .await?;

    send_task_update(ctx, claim.task_info, claim.activity_info).await?;
    push::send_applied_reward_pushes(
        ctx,
        player_id,
        claim.rewards,
        claim.material_changes,
        Some(task_type_reward_approach(type_id)),
    )
    .await?;

    ctx.send_reply(CmdId::GetTaskActivityBonusCmd, claim.reply, 0, req.up_tag)
        .await
}

pub async fn on_finish_read_task(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = FinishReadTaskRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let (reply, task) = ctx.player_mut()?.tasks.finish_read(db, msg.task_id).await?;

    if let Some(task) = task {
        send_task_update(ctx, vec![task], Vec::new()).await?;
    }

    ctx.send_reply(CmdId::FinishReadTaskCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_refresh_online_task(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = RefreshOnlineTaskRequest::decode(&req.data[..])?;
    let reply = tasks::refresh_online_task(msg.id);

    ctx.send_reply(CmdId::RefreshOnlineTaskCmd, reply, 0, req.up_tag)
        .await
}

async fn send_task_update(
    ctx: &mut ConnectionContext,
    task_info: Vec<sonettobuf::Task>,
    activity_info: Vec<sonettobuf::TaskActivityInfo>,
) -> Result<(), AppError> {
    if task_info.is_empty() && activity_info.is_empty() {
        return Ok(());
    }

    let red_dot_types = tasks::recurring_red_dot_types(task_info.iter().map(|task| {
        (
            task.r#type.unwrap_or_default(),
            task.has_finished,
            task.finish_count.unwrap_or_default(),
        )
    }));
    ctx.notify(
        CmdId::UpdateTaskPushCmd,
        UpdateTaskPush {
            task_info,
            activity_info: last_activity_by_type(activity_info),
        },
    )
    .await?;
    task_events::notify_task_red_dots(ctx, red_dot_types).await
}

async fn send_act233_bp_score_updates(
    ctx: &mut ConnectionContext,
    updates: &[database::db::game::act233_bp::Act233BpScoreUpdate],
) -> Result<(), AppError> {
    for update in updates {
        ctx.notify(
            CmdId::Act233BpScoreUpdatePushCmd,
            Act233BpScoreUpdatePush {
                activity_id: Some(update.activity_id),
                bp_id: Some(update.bp_id),
                score: Some(update.score),
            },
        )
        .await?;
    }

    Ok(())
}

fn task_reward_approach(tasks: &[sonettobuf::Task]) -> MaterialGetApproach {
    let is_recurring = !tasks.is_empty()
        && tasks
            .iter()
            .all(|task| is_recurring_task_type(task.r#type.unwrap_or_default()));

    if is_recurring {
        MaterialGetApproach::TaskAct
    } else {
        MaterialGetApproach::Task
    }
}

fn task_type_reward_approach(type_id: i32) -> MaterialGetApproach {
    if is_recurring_task_type(type_id) {
        MaterialGetApproach::TaskAct
    } else {
        MaterialGetApproach::Task
    }
}

fn is_recurring_task_type(type_id: i32) -> bool {
    matches!(
        TaskType::from_id(type_id),
        Some(TaskType::Daily | TaskType::Weekly)
    )
}

fn last_activity_by_type(
    activity_info: Vec<sonettobuf::TaskActivityInfo>,
) -> Vec<sonettobuf::TaskActivityInfo> {
    let mut by_type = std::collections::BTreeMap::new();
    for activity in activity_info {
        by_type.insert(activity.type_id, activity);
    }
    by_type.into_values().collect()
}

async fn notify_task_finish_events(
    ctx: &mut ConnectionContext,
    player_id: i64,
    task_ids: &[i32],
) -> Result<(), AppError> {
    for task_id in task_ids {
        task_events::notify(ctx, player_id, TaskEvent::TaskFinish { task_id: *task_id }).await?;
    }
    Ok(())
}

async fn send_bp_task_red_dot_update(
    ctx: &mut ConnectionContext,
    tasks: &[sonettobuf::Task],
) -> Result<(), AppError> {
    if !bp::has_task_red_dot(tasks) {
        return Ok(());
    }

    let red_dot_groups = ctx
        .player()?
        .red_dot
        .battle_pass_groups(ctx.state.db)
        .await?;
    push::send_red_dot_groups(ctx, red_dot_groups).await
}
