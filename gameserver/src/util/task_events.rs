use crate::{error::AppError, net::context::ConnectionContext};
use logic::{
    task::{TaskEvent, TaskType, UserTask},
    types::red_dot_id::RedDotId,
};
use sonettobuf::{CmdId, RedDotGroup, RedDotInfo, UpdateAchievementPush, UpdateTaskPush};
use std::{collections::BTreeMap, future::Future, pin::Pin};

use super::push;

pub async fn notify_tasks(
    ctx: &mut ConnectionContext,
    tasks: Vec<UserTask>,
) -> Result<(), AppError> {
    if tasks.is_empty() {
        return Ok(());
    }

    let activity_info = ctx.player()?.tasks.activity_info(ctx.state.db).await?;
    for (family, tasks) in task_push_groups(tasks) {
        let red_dot_types = logic::task::recurring_red_dot_types(
            tasks
                .iter()
                .map(|task| (task.type_id, task.has_finished, task.finish_count)),
        );
        ctx.notify(
            CmdId::UpdateTaskPushCmd,
            UpdateTaskPush {
                task_info: tasks.into_iter().map(Into::into).collect(),
                activity_info: activity_info.clone(),
            },
        )
        .await?;
        notify_task_family_red_dots(ctx, family, red_dot_types).await?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum TaskPushFamily {
    ActBp(i32),
    VersionActivity,
    BattlePass,
    Other,
}

fn task_push_groups(tasks: Vec<UserTask>) -> Vec<(TaskPushFamily, Vec<UserTask>)> {
    let mut groups = BTreeMap::<TaskPushFamily, Vec<UserTask>>::new();
    for task in tasks {
        let family = match TaskType::from_id(task.type_id) {
            Some(TaskType::ActBp) => TaskPushFamily::ActBp(task.activity_id),
            Some(TaskType::VersionActivity) => TaskPushFamily::VersionActivity,
            Some(TaskType::BattlePass) => TaskPushFamily::BattlePass,
            _ => TaskPushFamily::Other,
        };
        groups.entry(family).or_default().push(task);
    }
    groups.into_iter().collect()
}

fn notify_task_family_red_dots<'a>(
    ctx: &'a mut ConnectionContext,
    family: TaskPushFamily,
    recurring_types: Vec<i32>,
) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + 'a>> {
    Box::pin(async move {
        let groups = match family {
            TaskPushFamily::ActBp(activity_id) => {
                ctx.player()?
                    .red_dot
                    .act233_groups(ctx.state.db, activity_id)
                    .await?
            }
            TaskPushFamily::VersionActivity => vec![RedDotGroup {
                define_id: RedDotId::CommandStationTaskNormal.id(),
                infos: vec![RedDotInfo {
                    id: 0,
                    value: 1,
                    time: Some(0),
                    ext: None,
                }],
                replace_all: Some(true),
            }],
            TaskPushFamily::BattlePass => {
                ctx.player()?
                    .red_dot
                    .battle_pass_groups(ctx.state.db)
                    .await?
            }
            TaskPushFamily::Other => {
                notify_task_red_dots(ctx, recurring_types).await?;
                return Ok(());
            }
        };
        push::send_red_dot_groups(ctx, groups).await
    })
}

pub async fn notify_task_red_dots(
    ctx: &mut ConnectionContext,
    type_ids: Vec<i32>,
) -> Result<(), AppError> {
    for type_id in type_ids {
        let Some(red_dot) = ctx
            .player()?
            .tasks
            .recurring_red_dot(ctx.state.db, type_id)
            .await?
        else {
            continue;
        };
        push::send_red_dot_value_push(
            ctx,
            red_dot.define_id,
            vec![0],
            false,
            red_dot.value,
            red_dot.expiry,
        )
        .await?;
    }
    Ok(())
}

pub async fn notify(
    ctx: &mut ConnectionContext,
    player_id: i64,
    event: TaskEvent,
) -> Result<(), AppError> {
    let db = ctx.state.db;
    let updated_tasks = ctx.player_mut()?.tasks.sync_event(db, event).await?;
    let updated_achievements = ctx
        .player_mut()?
        .collection
        .sync_task_event(db, event)
        .await?;

    notify_tasks(ctx, updated_tasks).await?;

    if !updated_achievements.is_empty() {
        ctx.notify(
            CmdId::UpdateAchievementPushCmd,
            UpdateAchievementPush {
                infos: updated_achievements.into_iter().map(Into::into).collect(),
            },
        )
        .await?;
    }

    if let Some(count) = event.hero_touch_count() {
        ctx.player()?
            .profile
            .increment_hero_cover_times(ctx.state.db, count)
            .await?;
        crate::util::push::send_player_card_info_push(ctx, player_id).await?;
    }

    Ok(())
}
