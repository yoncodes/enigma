use crate::{bp, error::AppError, reward, room::RoomManager, types::red_dot_id::RedDotId};
use database::db::game::tasks as task_db;
pub use database::db::game::tasks::{ProductionLineAction, TaskEvent, TaskType};
pub use database::models::game::tasks::UserTask;
use sonettobuf::{
    FinishAllTaskReply, FinishReadTaskReply, FinishTaskReply, GetTaskActivityBonusReply,
    GetTaskInfoReply, RefreshOnlineTaskReply, Task, TaskActivityInfo,
};
use sqlx::SqlitePool;
use std::collections::HashMap;

mod rewards;

#[cfg(test)]
mod test;

use rewards::{
    add_claim_activity_in_transaction, parse_task_reward, task_activity_bonus, task_rewards,
};

#[derive(Clone, Debug)]
pub struct TaskManager {
    player_id: i64,
    tasks: HashMap<(i32, i32), UserTask>,
}

impl TaskManager {
    pub fn new(player_id: i64) -> Self {
        Self {
            player_id,
            tasks: HashMap::new(),
        }
    }

    pub async fn sync_login(
        &mut self,
        db: &SqlitePool,
        reset_daily: bool,
    ) -> Result<Vec<UserTask>, AppError> {
        let tasks = task_db::sync_login_tasks(db, self.player_id, reset_daily).await?;
        self.cache_tasks(&tasks);
        Ok(tasks)
    }

    pub async fn sync_event(
        &mut self,
        db: &SqlitePool,
        event: TaskEvent,
    ) -> Result<Vec<UserTask>, AppError> {
        let tasks = task_db::sync_event_tasks(db, self.player_id, event).await?;
        self.cache_tasks(&tasks);
        Ok(tasks)
    }

    pub async fn activity_info(&self, db: &SqlitePool) -> Result<Vec<TaskActivityInfo>, AppError> {
        Ok(task_db::list_activity(db, self.player_id, Vec::new())
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub async fn get_info(
        &mut self,
        db: &SqlitePool,
        type_ids: Vec<u32>,
    ) -> Result<GetTaskInfoReply, AppError> {
        let db_type_ids = type_ids
            .iter()
            .map(|type_id| *type_id as i32)
            .collect::<Vec<_>>();
        if db_type_ids.is_empty() || db_type_ids.contains(&task_db::TaskType::Room.id()) {
            RoomManager::new(self.player_id)
                .sync_room_tasks(db, config::configs::get())
                .await?;
        }
        let tasks = task_db::list_by_types(db, self.player_id, db_type_ids.clone()).await?;
        self.cache_tasks(&tasks);
        let activity = task_db::list_activity(db, self.player_id, db_type_ids)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();

        Ok(GetTaskInfoReply {
            task_info: tasks.into_iter().map(Into::into).collect(),
            activity_info: activity,
            type_ids,
        })
    }

    pub async fn finish(
        &mut self,
        db: &SqlitePool,
        task_id: i32,
    ) -> Result<TaskClaim<FinishTaskReply>, AppError> {
        if config::configs::get().task_room.get(task_id).is_some() {
            RoomManager::new(self.player_id)
                .sync_room_tasks(db, config::configs::get())
                .await?;
        }
        let task = task_db::get_by_id(db, self.player_id, task_id)
            .await?
            .ok_or(AppError::InvalidRequest)?;
        let mut tx = db.begin().await?;
        let task = task_db::finish_task_in_transaction(&mut tx, &task)
            .await?
            .ok_or(AppError::InvalidRequest)?;
        let (activity, mut reward_set) =
            add_claim_activity_in_transaction(&mut tx, self.player_id, &task).await?;
        reward_set.extend(task_rewards(task.type_id, task.task_id));
        add_battle_pass_score(&mut reward_set, std::slice::from_ref(&task));
        let material_changes = reward_set.material_changes();
        let rewards = reward::apply_in_transaction(&mut tx, db, self.player_id, reward_set).await?;
        tx.commit().await?;
        self.cache_task(task.clone());

        Ok(TaskClaim {
            reply: FinishTaskReply {
                id: Some(task.task_id),
                finish_count: Some(task.finish_count),
            },
            task_info: vec![task.into()],
            activity_info: activity.into_iter().map(Into::into).collect(),
            rewards,
            material_changes,
        })
    }

    pub async fn finish_all(
        &mut self,
        db: &SqlitePool,
        type_id: i32,
        min_type_id: Option<i32>,
        task_ids: Vec<i32>,
        activity_id: Option<i32>,
    ) -> Result<TaskClaim<FinishAllTaskReply>, AppError> {
        if type_id == task_db::TaskType::Room.id() {
            RoomManager::new(self.player_id)
                .sync_room_tasks(db, config::configs::get())
                .await?;
        }
        let claimable = task_db::claimable_tasks(
            db,
            self.player_id,
            type_id,
            min_type_id,
            activity_id,
            task_ids.clone(),
        )
        .await?;
        let mut tx = db.begin().await?;
        let tasks = task_db::finish_tasks_in_transaction(&mut tx, &claimable)
            .await?
            .ok_or(AppError::InvalidRequest)?;

        let mut activity = Vec::new();
        let mut reward_set = reward::RewardSet::default();
        for task in &tasks {
            let (updated_activity, activity_rewards) =
                add_claim_activity_in_transaction(&mut tx, self.player_id, task).await?;
            activity.extend(updated_activity);
            reward_set.extend(activity_rewards);
            reward_set.extend(task_rewards(task.type_id, task.task_id));
        }
        add_battle_pass_score(&mut reward_set, &tasks);

        let material_changes = reward_set.material_changes();
        let rewards = reward::apply_in_transaction(&mut tx, db, self.player_id, reward_set).await?;
        tx.commit().await?;
        self.cache_tasks(&tasks);

        Ok(TaskClaim {
            reply: FinishAllTaskReply {
                type_id: Some(type_id),
                min_type_id: Some(min_type_id.unwrap_or_default()),
                task_ids,
                activity_id,
            },
            task_info: tasks.into_iter().map(Into::into).collect(),
            activity_info: activity.into_iter().map(Into::into).collect(),
            rewards,
            material_changes,
        })
    }

    pub async fn get_activity_bonus(
        &mut self,
        db: &SqlitePool,
        type_id: i32,
        define_id: i32,
    ) -> Result<TaskClaim<GetTaskActivityBonusReply>, AppError> {
        let mut activity_info = Vec::new();
        let mut reward_set = reward::RewardSet::default();

        if let Some(bonus) = task_activity_bonus(type_id, define_id) {
            let mut tx = db.begin().await?;
            let (activity, claimed) = task_db::claim_activity_bonus_in_transaction(
                &mut tx,
                self.player_id,
                type_id,
                define_id,
                bonus.need_activity,
            )
            .await?;
            activity_info.push(activity.into());

            if claimed {
                reward_set = parse_task_reward(&bonus.bonus);
            }
            let material_changes = reward_set.material_changes();
            let rewards =
                reward::apply_in_transaction(&mut tx, db, self.player_id, reward_set).await?;
            tx.commit().await?;

            return Ok(TaskClaim {
                reply: GetTaskActivityBonusReply {
                    type_id: Some(type_id),
                    define_id: Some(define_id),
                },
                task_info: Vec::new(),
                activity_info,
                rewards,
                material_changes,
            });
        }

        let material_changes = reward_set.material_changes();
        let rewards = reward::apply(db, self.player_id, reward_set).await?;

        Ok(TaskClaim {
            reply: GetTaskActivityBonusReply {
                type_id: Some(type_id),
                define_id: Some(define_id),
            },
            task_info: Vec::new(),
            activity_info,
            rewards,
            material_changes,
        })
    }

    pub async fn finish_read(
        &mut self,
        db: &SqlitePool,
        task_id: Option<i32>,
    ) -> Result<(FinishReadTaskReply, Option<Task>), AppError> {
        let task_id = task_id.ok_or(AppError::InvalidRequest)?;
        let task = task_db::read_task(db, self.player_id, task_id).await?;
        if let Some(task) = &task {
            self.cache_task(task.clone());
        }

        Ok((
            FinishReadTaskReply {
                task_id: Some(task_id),
            },
            task.map(Into::into),
        ))
    }

    pub async fn recurring_red_dot(
        &self,
        db: &SqlitePool,
        type_id: i32,
    ) -> Result<Option<TaskRedDot>, AppError> {
        let Some((task_type, define_id)) = task_red_dot_route(type_id) else {
            return Ok(None);
        };
        let expiry = task_db::claimable_expiry(db, self.player_id, task_type).await?;
        Ok(Some(TaskRedDot {
            define_id,
            value: i32::from(expiry.is_some()),
            expiry: expiry.unwrap_or_default(),
        }))
    }

    fn cache_tasks(&mut self, tasks: &[UserTask]) {
        for task in tasks {
            self.cache_task(task.clone());
        }
    }

    fn cache_task(&mut self, task: UserTask) {
        self.tasks.insert((task.type_id, task.task_id), task);
    }
}

pub fn recurring_red_dot_types(tasks: impl IntoIterator<Item = (i32, bool, i32)>) -> Vec<i32> {
    let mut type_ids = tasks
        .into_iter()
        .filter(|(_, has_finished, finish_count)| *has_finished || *finish_count > 0)
        .filter_map(|(type_id, _, _)| task_red_dot_route(type_id).map(|_| type_id))
        .collect::<Vec<_>>();
    type_ids.sort_unstable();
    type_ids.dedup();
    type_ids
}

fn task_red_dot_route(type_id: i32) -> Option<(task_db::TaskType, i32)> {
    match task_db::TaskType::from_id(type_id)? {
        task_db::TaskType::Daily => Some((task_db::TaskType::Daily, RedDotId::DailyTask.id())),
        task_db::TaskType::Weekly => Some((task_db::TaskType::Weekly, RedDotId::WeeklyTask.id())),
        _ => None,
    }
}

fn add_battle_pass_score(rewards: &mut reward::RewardSet, tasks: &[UserTask]) {
    let Some(bp_id) = task_db::current_battle_pass_id() else {
        return;
    };
    let score = bp::task_score_from_models(bp_id, tasks);
    if score > 0 {
        rewards.bp_scores.push((bp_id, score));
    }
}

pub struct TaskClaim<T> {
    pub reply: T,
    pub task_info: Vec<Task>,
    pub activity_info: Vec<TaskActivityInfo>,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
}

pub struct TaskRedDot {
    pub define_id: i32,
    pub value: i32,
    pub expiry: i32,
}

pub fn refresh_online_task(id: Option<i32>) -> RefreshOnlineTaskReply {
    RefreshOnlineTaskReply { id }
}
