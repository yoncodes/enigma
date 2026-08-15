use crate::models::game::tasks::UserTask;
use common::time::ServerTime;
use sqlx::{Sqlite, SqliteConnection, SqlitePool, Transaction};

use super::battle_pass;

mod activity;
mod event;

pub use activity::{
    add_activity, add_activity_in_transaction, claim_activity_bonus,
    claim_activity_bonus_in_transaction, list_activity,
};
pub use event::{ProductionLineAction, TaskEvent, sync_event_tasks};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskType {
    Daily,
    Weekly,
    Achievement,
    Novice,
    Room,
    WeekWalk,
    Activity106,
    Season,
    ActivityDungeon,
    BattlePass,
    Activity119,
    ActivityShow,
    Activity125,
    Activity128,
    Activity180,
    Activity189,
    Activity194,
    AssassinOutside,
    Odyssey,
    Activity210,
    BpOperAct,
    ActBp,
    Activity220,
    MiniParty,
    ObserverBox,
    Turnback,
    NecrologistStory,
}

impl TaskType {
    pub const fn id(self) -> i32 {
        match self {
            Self::Daily => 1,
            Self::Weekly => 2,
            Self::Achievement => 3,
            Self::Novice => 4,
            Self::Room => 6,
            Self::WeekWalk => 7,
            Self::Activity106 => 8,
            Self::Season => 9,
            Self::ActivityDungeon => 11,
            Self::BattlePass => 10,
            Self::Activity119 => 14,
            Self::ActivityShow => 16,
            Self::Activity125 => 45,
            Self::Activity128 => 22,
            Self::Activity180 => 44,
            Self::Activity189 => 53,
            Self::Activity194 => 58,
            Self::AssassinOutside => 59,
            Self::Odyssey => 60,
            Self::Activity210 => 67,
            Self::BpOperAct => 70,
            Self::ActBp => 79,
            Self::Activity220 => 71,
            Self::MiniParty => 73,
            Self::ObserverBox => 74,
            Self::Turnback => 18,
            Self::NecrologistStory => 65,
        }
    }

    pub const fn from_id(id: i32) -> Option<Self> {
        match id {
            1 => Some(Self::Daily),
            2 => Some(Self::Weekly),
            3 => Some(Self::Achievement),
            4 => Some(Self::Novice),
            6 => Some(Self::Room),
            7 => Some(Self::WeekWalk),
            8 => Some(Self::Activity106),
            9 => Some(Self::Season),
            10 => Some(Self::BattlePass),
            11 => Some(Self::ActivityDungeon),
            14 => Some(Self::Activity119),
            16 => Some(Self::ActivityShow),
            18 => Some(Self::Turnback),
            22 => Some(Self::Activity128),
            44 => Some(Self::Activity180),
            45 => Some(Self::Activity125),
            53 => Some(Self::Activity189),
            58 => Some(Self::Activity194),
            59 => Some(Self::AssassinOutside),
            60 => Some(Self::Odyssey),
            65 => Some(Self::NecrologistStory),
            67 => Some(Self::Activity210),
            70 => Some(Self::BpOperAct),
            79 => Some(Self::ActBp),
            71 => Some(Self::Activity220),
            73 => Some(Self::MiniParty),
            74 => Some(Self::ObserverBox),
            _ => None,
        }
    }

    pub const fn all() -> &'static [Self] {
        &[
            Self::Daily,
            Self::Weekly,
            Self::Achievement,
            Self::Novice,
            Self::Room,
            Self::WeekWalk,
            Self::Activity106,
            Self::Season,
            Self::ActivityDungeon,
            Self::BattlePass,
            Self::Activity119,
            Self::ActivityShow,
            Self::Activity125,
            Self::Activity128,
            Self::Activity180,
            Self::Activity189,
            Self::Activity194,
            Self::AssassinOutside,
            Self::Odyssey,
            Self::Activity210,
            Self::BpOperAct,
            Self::ActBp,
            Self::Activity220,
            Self::MiniParty,
            Self::ObserverBox,
            Self::NecrologistStory,
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskLoopType {
    Daily,
    Weekly,
    Permanent,
    HalfMonth,
    Appoint,
}

impl TaskLoopType {
    pub const fn id(self) -> i32 {
        match self {
            Self::Daily => 1,
            Self::Weekly => 2,
            Self::Permanent => 3,
            Self::HalfMonth => 4,
            Self::Appoint => 5,
        }
    }

    pub const fn from_id(id: i32) -> Option<Self> {
        match id {
            1 => Some(Self::Daily),
            2 => Some(Self::Weekly),
            3 => Some(Self::Permanent),
            4 => Some(Self::HalfMonth),
            5 => Some(Self::Appoint),
            _ => None,
        }
    }
}

pub async fn ensure_tasks_for_type(
    pool: &SqlitePool,
    user_id: i64,
    task_type: TaskType,
) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    ensure_tasks_for_type_in_transaction(&mut tx, user_id, task_type).await?;
    tx.commit().await
}

async fn ensure_tasks_for_type_in_transaction(
    pool: &mut SqliteConnection,
    user_id: i64,
    task_type: TaskType,
) -> sqlx::Result<()> {
    let tables = config::configs::get();

    match task_type {
        TaskType::Daily => {
            for task in tables.online_daily_tasks() {
                ensure_task(
                    pool,
                    NewTask::new(user_id, task_type.id(), task.id, task.activity_id, 0),
                )
                .await?;
            }
        }
        TaskType::Weekly => {
            for task in tables.online_weekly_tasks() {
                ensure_task(
                    pool,
                    NewTask::new(user_id, task_type.id(), task.id, task.activity_id, 0),
                )
                .await?;
            }
        }
        TaskType::Achievement => {
            for task in tables
                .task_achievement
                .iter()
                .filter(|task| task.is_online != 0)
            {
                ensure_task(pool, NewTask::new(user_id, task_type.id(), task.id, 0, 0)).await?;
            }
        }
        TaskType::Novice => {
            for task in tables.online_guide_tasks() {
                ensure_task(
                    pool,
                    NewTask::new(user_id, task_type.id(), task.id, 0, task.min_type_id),
                )
                .await?;
            }
        }
        TaskType::Room => {
            for task in tables.online_room_tasks() {
                ensure_task(pool, NewTask::new(user_id, task_type.id(), task.id, 0, 0)).await?;
            }
        }
        TaskType::WeekWalk => {
            for task in tables.task_weekwalk.iter() {
                ensure_task(
                    pool,
                    NewTask::new(user_id, task_type.id(), task.id, 0, task.min_type_id),
                )
                .await?;
            }
        }
        TaskType::Activity106 => {
            ensure_config_tasks(
                pool,
                user_id,
                task_type.id(),
                tables
                    .activity106_task
                    .iter()
                    .map(|task| ConfigTask::online(task.id, task.is_online, task.activity_id)),
            )
            .await?;
        }
        TaskType::Season => {
            for task in tables.online_season_tasks() {
                ensure_task(
                    pool,
                    NewTask::new(
                        user_id,
                        task_type.id(),
                        task.id,
                        task.season_id,
                        task.min_type_id,
                    ),
                )
                .await?;
            }
        }
        TaskType::ActivityDungeon => {
            ensure_config_tasks(
                pool,
                user_id,
                task_type.id(),
                tables
                    .activity113_task
                    .iter()
                    .map(|task| ConfigTask::online(task.id, task.is_online, task.activity_id)),
            )
            .await?;
        }
        TaskType::BattlePass => {
            if let Some(bp_id) = current_battle_pass_id() {
                ensure_battle_pass_tasks_in_transaction(pool, user_id, bp_id).await?;
            }
        }
        TaskType::Activity119 => {
            ensure_config_tasks(
                pool,
                user_id,
                task_type.id(),
                tables
                    .activity119_task
                    .iter()
                    .map(|task| ConfigTask::online(task.id, task.is_online, task.activity_id)),
            )
            .await?;
        }
        TaskType::ActivityShow => {
            for task in tables
                .task_activity_show
                .iter()
                .filter(|task| task.is_online != 0)
            {
                ensure_task(
                    pool,
                    NewTask::new(user_id, task_type.id(), task.id, task.activity_id, 0),
                )
                .await?;
            }
        }
        TaskType::Activity125 => {
            ensure_config_tasks(
                pool,
                user_id,
                task_type.id(),
                tables
                    .activity125_task
                    .iter()
                    .map(|task| ConfigTask::online(task.id, task.is_online, task.activity_id)),
            )
            .await?;
        }
        TaskType::Activity128 => {
            ensure_config_tasks(
                pool,
                user_id,
                task_type.id(),
                tables
                    .activity128_task
                    .iter()
                    .map(|task| ConfigTask::online(task.id, task.is_online, task.activity_id)),
            )
            .await?;
        }
        TaskType::Activity180 => {
            ensure_config_tasks(
                pool,
                user_id,
                task_type.id(),
                tables
                    .activity180_task
                    .iter()
                    .map(|task| ConfigTask::online(task.id, task.is_online, task.activity_id)),
            )
            .await?;
        }
        TaskType::Activity189 => {
            ensure_config_tasks(
                pool,
                user_id,
                task_type.id(),
                tables
                    .activity189_task
                    .iter()
                    .map(|task| ConfigTask::online(task.id, task.is_online, task.activity_id)),
            )
            .await?;
        }
        TaskType::Activity194 => {
            ensure_config_tasks(
                pool,
                user_id,
                task_type.id(),
                tables
                    .activity194_task
                    .iter()
                    .map(|task| ConfigTask::online(task.id, task.is_online, task.activity_id)),
            )
            .await?;
        }
        TaskType::AssassinOutside => {
            ensure_config_tasks(
                pool,
                user_id,
                task_type.id(),
                tables
                    .assassin_task
                    .iter()
                    .map(|task| ConfigTask::online(task.id, task.is_online, task.activity_id)),
            )
            .await?;
        }
        TaskType::Odyssey => {
            ensure_config_tasks(
                pool,
                user_id,
                task_type.id(),
                tables
                    .odyssey_task
                    .iter()
                    .map(|task| ConfigTask::online(task.id, task.is_online, task.activity_id)),
            )
            .await?;
        }
        TaskType::Activity210 => {
            ensure_config_tasks(
                pool,
                user_id,
                task_type.id(),
                tables
                    .activity210_task
                    .iter()
                    .map(|task| ConfigTask::online(task.id, task.is_online, task.activity_id)),
            )
            .await?;
        }
        TaskType::BpOperAct => {
            if let Some(bp_id) = current_battle_pass_id() {
                ensure_bp_oper_act_tasks_in_transaction(pool, user_id, bp_id).await?;
            }
        }
        TaskType::ActBp => {
            let weekly_expiry = ServerTime::next_weekly_refresh_sec(ServerTime::now_ms());
            ensure_config_tasks(
                pool,
                user_id,
                task_type.id(),
                tables.activity233_task.iter().map(|task| {
                    ConfigTask::act_bp(
                        task.id,
                        task.is_online,
                        task.activity_id,
                        task.loop_type,
                        weekly_expiry,
                    )
                }),
            )
            .await?;
        }
        TaskType::Activity220 => {
            ensure_config_tasks(
                pool,
                user_id,
                task_type.id(),
                tables
                    .activity220_task
                    .iter()
                    .map(|task| ConfigTask::online(task.id, task.is_online, task.activity_id)),
            )
            .await?;
        }
        TaskType::MiniParty => {
            ensure_config_tasks(
                pool,
                user_id,
                task_type.id(),
                tables.activity223_task.iter().map(|task| {
                    ConfigTask::looped(task.id, task.is_online, task.activity_id, task.loop_type)
                }),
            )
            .await?;
        }
        TaskType::ObserverBox => {
            ensure_config_tasks(
                pool,
                user_id,
                task_type.id(),
                tables.activity226_task.iter().map(|task| {
                    ConfigTask::looped(task.id, task.is_online, task.activity_id, task.loop_type)
                }),
            )
            .await?;
        }
        TaskType::Turnback => {}
        TaskType::NecrologistStory => {
            for task in tables
                .hero_story_task
                .iter()
                .filter(|task| task.is_online != 0)
            {
                ensure_task(
                    pool,
                    NewTask::new(
                        user_id,
                        task_type.id(),
                        task.id,
                        task.activity_id,
                        task.story_id,
                    ),
                )
                .await?;
            }
        }
    }

    Ok(())
}

pub(crate) async fn seed_configured_tasks_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
) -> sqlx::Result<()> {
    for task_type in TaskType::all() {
        ensure_tasks_for_type_in_transaction(&mut *tx, user_id, *task_type).await?;
    }
    Ok(())
}

pub async fn ensure_turnback_tasks(
    pool: &SqlitePool,
    user_id: i64,
    turnback_id: i32,
    tables: &config::GameDB,
) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    ensure_turnback_tasks_in_transaction(&mut tx, user_id, turnback_id, tables).await?;
    tx.commit().await
}

async fn ensure_turnback_tasks_in_transaction(
    conn: &mut SqliteConnection,
    user_id: i64,
    turnback_id: i32,
    tables: &config::GameDB,
) -> sqlx::Result<()> {
    sqlx::query(
        "DELETE FROM user_tasks
         WHERE user_id = ? AND type_id = ? AND activity_id != ?",
    )
    .bind(user_id)
    .bind(TaskType::Turnback.id())
    .bind(turnback_id)
    .execute(&mut *conn)
    .await?;

    for task in tables
        .turnback_task
        .iter()
        .filter(|task| task.turnback_id == turnback_id && task.is_online != 0)
    {
        ensure_task(
            conn,
            NewTask {
                user_id,
                task_type_id: TaskType::Turnback.id(),
                task_id: task.id,
                expiry_time: 0,
                min_type_id: TaskLoopType::from_id(task.loop_type)
                    .map(TaskLoopType::id)
                    .unwrap_or(task.loop_type),
                activity_id: task.turnback_id,
            },
        )
        .await?;
    }

    Ok(())
}

pub async fn ensure_battle_pass_tasks(
    pool: &SqlitePool,
    user_id: i64,
    bp_id: i32,
) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    ensure_battle_pass_tasks_in_transaction(&mut tx, user_id, bp_id).await?;
    tx.commit().await
}

pub async fn ensure_bp_oper_act_tasks(
    pool: &SqlitePool,
    user_id: i64,
    bp_id: i32,
) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    ensure_bp_oper_act_tasks_in_transaction(&mut tx, user_id, bp_id).await?;
    tx.commit().await
}

async fn ensure_bp_oper_act_tasks_in_transaction(
    conn: &mut SqliteConnection,
    user_id: i64,
    bp_id: i32,
) -> sqlx::Result<()> {
    ensure_config_tasks(
        conn,
        user_id,
        TaskType::BpOperAct.id(),
        config::configs::get()
            .activity214_task
            .iter()
            .filter(|task| task.bp_id == bp_id)
            .map(|task| ConfigTask {
                task_id: task.id,
                is_online: task.is_online != 0,
                activity_id: task.activity_id,
                min_type_id: TaskLoopType::from_id(task.loop_type)
                    .map(TaskLoopType::id)
                    .unwrap_or(task.loop_type),
                expiry_time: parse_time(&task.end_time),
            }),
    )
    .await
}

async fn ensure_battle_pass_tasks_in_transaction(
    conn: &mut SqliteConnection,
    user_id: i64,
    bp_id: i32,
) -> sqlx::Result<()> {
    sqlx::query(
        "DELETE FROM user_tasks
         WHERE user_id = ? AND type_id = ? AND activity_id != ?",
    )
    .bind(user_id)
    .bind(TaskType::BattlePass.id())
    .bind(bp_id)
    .execute(&mut *conn)
    .await?;

    for task in config::configs::get()
        .bp_task
        .iter()
        .filter(|task| task.bp_id == bp_id && task.is_online != 0)
    {
        ensure_task(
            conn,
            NewTask {
                user_id,
                task_type_id: TaskType::BattlePass.id(),
                task_id: task.id,
                expiry_time: parse_time(&task.end_time),
                min_type_id: TaskLoopType::from_id(task.loop_type)
                    .map(TaskLoopType::id)
                    .unwrap_or(task.loop_type),
                activity_id: task.bp_id,
            },
        )
        .await?;
    }

    Ok(())
}

pub async fn list_by_types(
    pool: &SqlitePool,
    user_id: i64,
    type_ids: Vec<i32>,
) -> sqlx::Result<Vec<UserTask>> {
    if type_ids.is_empty() {
        return sqlx::query_as::<_, UserTask>(
            "SELECT user_id, type_id, task_id, progress, has_finished, finish_count,
                    expiry_time, min_type_id, activity_id, created_at, updated_at
             FROM user_tasks
             WHERE user_id = ?
             ORDER BY type_id, task_id",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await;
    }

    let placeholders = std::iter::repeat_n("?", type_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT user_id, type_id, task_id, progress, has_finished, finish_count,
                expiry_time, min_type_id, activity_id, created_at, updated_at
         FROM user_tasks
         WHERE user_id = ? AND type_id IN ({})
         ORDER BY type_id, task_id",
        placeholders
    );
    let mut query = sqlx::query_as::<_, UserTask>(&sql).bind(user_id);
    for type_id in type_ids {
        query = query.bind(type_id);
    }
    query.fetch_all(pool).await
}

pub async fn list_battle_pass(
    pool: &SqlitePool,
    user_id: i64,
    bp_id: i32,
) -> sqlx::Result<Vec<UserTask>> {
    sqlx::query_as::<_, UserTask>(
        "SELECT user_id, type_id, task_id, progress, has_finished, finish_count,
                expiry_time, min_type_id, activity_id, created_at, updated_at
         FROM user_tasks
         WHERE user_id = ? AND type_id = ? AND activity_id = ?
         ORDER BY min_type_id, task_id",
    )
    .bind(user_id)
    .bind(TaskType::BattlePass.id())
    .bind(bp_id)
    .fetch_all(pool)
    .await
}

pub async fn list_act_bp(
    pool: &SqlitePool,
    user_id: i64,
    activity_id: i32,
) -> sqlx::Result<Vec<UserTask>> {
    sqlx::query_as::<_, UserTask>(
        "SELECT user_id, type_id, task_id, progress, has_finished, finish_count,
                expiry_time, min_type_id, activity_id, created_at, updated_at
         FROM user_tasks
         WHERE user_id = ? AND type_id = ? AND activity_id = ?
         ORDER BY min_type_id, task_id",
    )
    .bind(user_id)
    .bind(TaskType::ActBp.id())
    .bind(activity_id)
    .fetch_all(pool)
    .await
}

pub async fn list_turnback(
    pool: &SqlitePool,
    user_id: i64,
    turnback_id: i32,
) -> sqlx::Result<Vec<UserTask>> {
    sqlx::query_as::<_, UserTask>(
        "SELECT user_id, type_id, task_id, progress, has_finished, finish_count,
                expiry_time, min_type_id, activity_id, created_at, updated_at
         FROM user_tasks
         WHERE user_id = ? AND type_id = ? AND activity_id = ?
         ORDER BY min_type_id, task_id",
    )
    .bind(user_id)
    .bind(TaskType::Turnback.id())
    .bind(turnback_id)
    .fetch_all(pool)
    .await
}

pub async fn reset_daily_tasks(pool: &SqlitePool, user_id: i64) -> sqlx::Result<()> {
    let daily_ids = config::configs::get()
        .task_daily
        .iter()
        .filter(|task| task.is_online != 0)
        .map(|task| task.id)
        .collect::<Vec<_>>();
    let bp_daily_ids = config::configs::get()
        .bp_task
        .iter()
        .filter(|task| task.is_online != 0 && task.loop_type == TaskLoopType::Daily.id())
        .map(|task| task.id)
        .collect::<Vec<_>>();

    reset_task_ids(
        pool,
        user_id,
        vec![
            (TaskType::Daily.id(), daily_ids),
            (TaskType::BattlePass.id(), bp_daily_ids),
        ],
    )
    .await
}

pub async fn reset_weekly_tasks(pool: &SqlitePool, user_id: i64) -> sqlx::Result<()> {
    let weekly_ids = config::configs::get()
        .task_weekly
        .iter()
        .filter(|task| task.is_online != 0)
        .map(|task| task.id)
        .collect::<Vec<_>>();
    let weekwalk_ids = config::configs::get()
        .task_weekwalk
        .iter()
        .filter(|task| task.min_type_id == 2)
        .map(|task| task.id)
        .collect::<Vec<_>>();
    let bp_weekly_ids = config::configs::get()
        .bp_task
        .iter()
        .filter(|task| task.is_online != 0 && task.loop_type == TaskLoopType::Weekly.id())
        .map(|task| task.id)
        .collect::<Vec<_>>();
    let act_bp_weekly_ids = config::configs::get()
        .activity233_task
        .iter()
        .filter(|task| task.is_online != 0 && task.loop_type == TaskLoopType::Weekly.id())
        .map(|task| task.id)
        .collect::<Vec<_>>();

    reset_task_ids(
        pool,
        user_id,
        vec![
            (TaskType::Weekly.id(), weekly_ids),
            (TaskType::WeekWalk.id(), weekwalk_ids),
            (TaskType::BattlePass.id(), bp_weekly_ids),
            (TaskType::ActBp.id(), act_bp_weekly_ids),
        ],
    )
    .await
}

pub async fn finish_task(
    pool: &SqlitePool,
    user_id: i64,
    task_id: i32,
) -> sqlx::Result<Option<UserTask>> {
    let Some(task) = get_by_id(pool, user_id, task_id).await? else {
        return Ok(None);
    };
    if !task.has_finished || task.finish_count >= max_finish_count(task.type_id, task.task_id) {
        return Ok(None);
    }

    let now = ServerTime::now_ms();
    let result = sqlx::query(
        "UPDATE user_tasks
         SET finish_count = finish_count + 1, has_finished = 0, updated_at = ?
         WHERE user_id = ? AND type_id = ? AND task_id = ?",
    )
    .bind(now)
    .bind(user_id)
    .bind(task.type_id)
    .bind(task_id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Ok(None);
    }

    get_by_id(pool, user_id, task_id).await
}

pub async fn finish_task_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    task: &UserTask,
) -> sqlx::Result<Option<UserTask>> {
    if !task.has_finished || task.finish_count >= max_finish_count(task.type_id, task.task_id) {
        return Ok(None);
    }
    let now = ServerTime::now_ms();
    let result = sqlx::query(
        "UPDATE user_tasks SET finish_count = finish_count + 1, has_finished = 0, updated_at = ?
         WHERE user_id = ? AND type_id = ? AND task_id = ? AND finish_count = ?",
    )
    .bind(now)
    .bind(task.user_id)
    .bind(task.type_id)
    .bind(task.task_id)
    .bind(task.finish_count)
    .execute(&mut **tx)
    .await?;
    if result.rows_affected() != 1 {
        return Ok(None);
    }
    let mut updated = task.clone();
    updated.finish_count += 1;
    updated.has_finished = false;
    updated.updated_at = now;
    Ok(Some(updated))
}

pub async fn finish_tasks_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    tasks: &[UserTask],
) -> sqlx::Result<Option<Vec<UserTask>>> {
    let mut updated = Vec::with_capacity(tasks.len());
    for task in tasks {
        let Some(task) = finish_task_in_transaction(tx, task).await? else {
            return Ok(None);
        };
        updated.push(task);
    }
    Ok(Some(updated))
}

pub async fn finish_all(
    pool: &SqlitePool,
    user_id: i64,
    type_id: i32,
    min_type_id: Option<i32>,
    activity_id: Option<i32>,
    task_ids: Vec<i32>,
) -> sqlx::Result<Vec<UserTask>> {
    let claimable =
        claimable_tasks(pool, user_id, type_id, min_type_id, activity_id, task_ids).await?;
    let now = ServerTime::now_ms();

    for task in &claimable {
        sqlx::query(
            "UPDATE user_tasks
             SET finish_count = finish_count + 1, has_finished = 0, updated_at = ?
             WHERE user_id = ? AND type_id = ? AND task_id = ?",
        )
        .bind(now)
        .bind(user_id)
        .bind(task.type_id)
        .bind(task.task_id)
        .execute(pool)
        .await?;
    }

    Ok(claimable
        .into_iter()
        .map(|mut task| {
            task.finish_count += 1;
            task.has_finished = false;
            task.updated_at = now;
            task
        })
        .collect())
}

pub async fn read_task(
    pool: &SqlitePool,
    user_id: i64,
    task_id: i32,
) -> sqlx::Result<Option<UserTask>> {
    let Some(task) = get_by_id(pool, user_id, task_id).await? else {
        return Ok(None);
    };

    let now = ServerTime::now_ms();
    let has_finished = task.finish_count < max_finish_count(task.type_id, task.task_id);
    sqlx::query(
        "UPDATE user_tasks
         SET progress = CASE WHEN progress < 1 THEN 1 ELSE progress END,
             has_finished = ?,
             updated_at = ?
         WHERE user_id = ? AND type_id = ? AND task_id = ?",
    )
    .bind(has_finished)
    .bind(now)
    .bind(user_id)
    .bind(task.type_id)
    .bind(task_id)
    .execute(pool)
    .await?;

    get_by_id(pool, user_id, task_id).await
}

pub async fn set_progress(
    pool: &SqlitePool,
    user_id: i64,
    type_id: i32,
    task_id: i32,
    progress: i32,
    has_finished: bool,
) -> sqlx::Result<()> {
    let now = ServerTime::now_ms();
    sqlx::query(
        "UPDATE user_tasks
         SET progress = ?, has_finished = ?, updated_at = ?
         WHERE user_id = ? AND type_id = ? AND task_id = ?",
    )
    .bind(progress)
    .bind(has_finished)
    .bind(now)
    .bind(user_id)
    .bind(type_id)
    .bind(task_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn sync_progress(
    pool: &SqlitePool,
    user_id: i64,
    type_id: i32,
    task_id: i32,
    progress: i32,
    max_progress: i32,
) -> sqlx::Result<Option<UserTask>> {
    let Some(task) = get_by_type_and_id(pool, user_id, type_id, task_id).await? else {
        return Ok(None);
    };
    let max_progress = max_progress.max(1);
    let progress = progress.clamp(0, max_progress);
    let has_finished =
        progress >= max_progress && task.finish_count < max_finish_count(type_id, task_id);
    if progress == task.progress && has_finished == task.has_finished {
        return Ok(None);
    }

    set_progress(pool, user_id, type_id, task_id, progress, has_finished).await?;
    get_by_type_and_id(pool, user_id, type_id, task_id).await
}

pub async fn sync_login_tasks(
    pool: &SqlitePool,
    user_id: i64,
    is_new_day: bool,
) -> sqlx::Result<Vec<UserTask>> {
    ensure_tasks_for_type(pool, user_id, TaskType::ActBp).await?;
    let now = ServerTime::now_ms();
    let daily_expiry = ServerTime::next_daily_refresh_sec(now);
    let weekly_expiry = ServerTime::next_weekly_refresh_sec(now);
    set_type_expiry(pool, user_id, TaskType::Daily, daily_expiry).await?;
    set_type_expiry(pool, user_id, TaskType::Weekly, weekly_expiry).await?;
    set_type_expiry(pool, user_id, TaskType::WeekWalk, weekly_expiry).await?;
    set_task_loop_expiry(
        pool,
        user_id,
        TaskType::ActBp,
        TaskLoopType::Weekly,
        weekly_expiry,
    )
    .await?;
    set_task_loop_expiry(pool, user_id, TaskType::ActBp, TaskLoopType::Permanent, 0).await?;
    set_activity_expiry(pool, user_id, TaskType::Daily, daily_expiry).await?;
    set_activity_expiry(pool, user_id, TaskType::Weekly, weekly_expiry).await?;
    set_activity_expiry(pool, user_id, TaskType::WeekWalk, weekly_expiry).await?;

    let bp_id = current_battle_pass_id();
    let include_bp = match bp_id {
        Some(bp_id) => !battle_pass::score_maxed(pool, user_id, bp_id).await?,
        None => false,
    };
    let mut updated = Vec::new();
    for target in login_task_targets(bp_id.filter(|_| include_bp)) {
        let Some(task) = get_by_type_and_id(pool, user_id, target.type_id, target.task_id).await?
        else {
            continue;
        };

        let max_progress = target.max_progress.max(1);
        let progress = if is_new_day {
            task.progress + 1
        } else {
            task.progress.max(1)
        }
        .min(max_progress);
        let has_finished = progress >= max_progress
            && task.finish_count < max_finish_count(target.type_id, target.task_id);

        if progress == task.progress && has_finished == task.has_finished {
            continue;
        }

        set_progress(
            pool,
            user_id,
            target.type_id,
            target.task_id,
            progress,
            has_finished,
        )
        .await?;

        if let Some(task) =
            get_by_type_and_id(pool, user_id, target.type_id, target.task_id).await?
        {
            updated.push(task);
        }
    }

    Ok(updated)
}

async fn set_type_expiry(
    pool: &SqlitePool,
    user_id: i64,
    task_type: TaskType,
    expiry_time: i32,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE user_tasks SET expiry_time = ? WHERE user_id = ? AND type_id = ?")
        .bind(expiry_time)
        .bind(user_id)
        .bind(task_type.id())
        .execute(pool)
        .await?;
    Ok(())
}

async fn set_task_loop_expiry(
    pool: &SqlitePool,
    user_id: i64,
    task_type: TaskType,
    loop_type: TaskLoopType,
    expiry_time: i32,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE user_tasks
         SET expiry_time = ?
         WHERE user_id = ? AND type_id = ? AND min_type_id = ?",
    )
    .bind(expiry_time)
    .bind(user_id)
    .bind(task_type.id())
    .bind(loop_type.id())
    .execute(pool)
    .await?;
    Ok(())
}

async fn set_activity_expiry(
    pool: &SqlitePool,
    user_id: i64,
    task_type: TaskType,
    expiry_time: i32,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE user_task_activity SET expiry_time = ? WHERE user_id = ? AND type_id = ?")
        .bind(expiry_time)
        .bind(user_id)
        .bind(task_type.id())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn claimable_expiry(
    pool: &SqlitePool,
    user_id: i64,
    task_type: TaskType,
) -> sqlx::Result<Option<i32>> {
    Ok(
        claimable_tasks(pool, user_id, task_type.id(), None, None, Vec::new())
            .await?
            .into_iter()
            .map(|task| task.expiry_time)
            .max(),
    )
}

fn login_task_targets(bp_id: Option<i32>) -> Vec<LoginTaskTarget> {
    let tables = config::configs::get();
    let mut targets = Vec::new();

    targets.extend(
        tables
            .online_daily_tasks()
            .filter(|task| task.listener_type == "LoginDays")
            .map(|task| LoginTaskTarget::new(TaskType::Daily, task.id, task.max_progress)),
    );
    targets.extend(
        tables
            .online_weekly_tasks()
            .filter(|task| task.listener_type == "LoginDays")
            .map(|task| LoginTaskTarget::new(TaskType::Weekly, task.id, task.max_progress)),
    );

    if let Some(bp_id) = bp_id {
        targets.extend(
            tables
                .battle_pass_tasks(bp_id)
                .filter(|task| task.is_online != 0 && task.listener_type == "LoginDays")
                .map(|task| LoginTaskTarget::new(TaskType::BattlePass, task.id, task.max_progress)),
        );
        targets.extend(
            tables
                .activity214_task
                .iter()
                .filter(|task| {
                    task.bp_id == bp_id && task.is_online != 0 && task.listener_type == "LoginDays"
                })
                .map(|task| LoginTaskTarget::new(TaskType::BpOperAct, task.id, task.max_progress)),
        );
    }
    targets.extend(
        tables
            .activity125_task
            .iter()
            .filter(|task| task.is_online != 0 && task.listener_type == "LoginDays")
            .map(|task| LoginTaskTarget::new(TaskType::Activity125, task.id, task.max_progress)),
    );
    targets.extend(
        tables
            .activity233_task
            .iter()
            .filter(|task| task.is_online != 0 && task.listener_type == "LoginDays")
            .map(|task| LoginTaskTarget::new(TaskType::ActBp, task.id, task.max_progress)),
    );

    targets
}

struct LoginTaskTarget {
    type_id: i32,
    task_id: i32,
    max_progress: i32,
}

impl LoginTaskTarget {
    fn new(task_type: TaskType, task_id: i32, max_progress: i32) -> Self {
        Self {
            type_id: task_type.id(),
            task_id,
            max_progress,
        }
    }
}

async fn ensure_task(conn: &mut SqliteConnection, task: NewTask) -> sqlx::Result<()> {
    let now = ServerTime::now_ms();
    sqlx::query(
        "INSERT OR IGNORE INTO user_tasks
         (user_id, type_id, task_id, expiry_time, min_type_id, activity_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(task.user_id)
    .bind(task.task_type_id)
    .bind(task.task_id)
    .bind(task.expiry_time)
    .bind(task.min_type_id)
    .bind(task.activity_id)
    .bind(now)
    .bind(now)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub(super) async fn add_progress(
    pool: &SqlitePool,
    user_id: i64,
    type_id: i32,
    task_id: i32,
    delta: i32,
    max_progress: i32,
) -> sqlx::Result<Option<UserTask>> {
    let Some(task) = get_by_type_and_id(pool, user_id, type_id, task_id).await? else {
        return Ok(None);
    };

    let max_progress = max_progress.max(1);
    let progress = (task.progress + delta.max(1)).min(max_progress);
    let has_finished =
        progress >= max_progress && task.finish_count < max_finish_count(type_id, task_id);
    if progress == task.progress && has_finished == task.has_finished {
        return Ok(None);
    }

    set_progress(pool, user_id, type_id, task_id, progress, has_finished).await?;
    get_by_type_and_id(pool, user_id, type_id, task_id).await
}

pub async fn get_by_id(
    pool: &SqlitePool,
    user_id: i64,
    task_id: i32,
) -> sqlx::Result<Option<UserTask>> {
    sqlx::query_as::<_, UserTask>(
        "SELECT user_id, type_id, task_id, progress, has_finished, finish_count,
                expiry_time, min_type_id, activity_id, created_at, updated_at
         FROM user_tasks
         WHERE user_id = ? AND task_id = ?
         ORDER BY type_id
         LIMIT 1",
    )
    .bind(user_id)
    .bind(task_id)
    .fetch_optional(pool)
    .await
}

async fn get_by_type_and_id(
    pool: &SqlitePool,
    user_id: i64,
    type_id: i32,
    task_id: i32,
) -> sqlx::Result<Option<UserTask>> {
    sqlx::query_as::<_, UserTask>(
        "SELECT user_id, type_id, task_id, progress, has_finished, finish_count,
                expiry_time, min_type_id, activity_id, created_at, updated_at
         FROM user_tasks
         WHERE user_id = ? AND type_id = ? AND task_id = ?",
    )
    .bind(user_id)
    .bind(type_id)
    .bind(task_id)
    .fetch_optional(pool)
    .await
}

pub async fn claimable_tasks(
    pool: &SqlitePool,
    user_id: i64,
    type_id: i32,
    min_type_id: Option<i32>,
    activity_id: Option<i32>,
    task_ids: Vec<i32>,
) -> sqlx::Result<Vec<UserTask>> {
    let mut sql = String::from(
        "SELECT user_id, type_id, task_id, progress, has_finished, finish_count,
                expiry_time, min_type_id, activity_id, created_at, updated_at
         FROM user_tasks
         WHERE user_id = ? AND type_id = ? AND has_finished = 1",
    );
    if min_type_id.is_some() {
        sql.push_str(" AND min_type_id = ?");
    }
    if activity_id.is_some() {
        sql.push_str(" AND activity_id = ?");
    }
    if !task_ids.is_empty() {
        sql.push_str(" AND task_id IN (");
        sql.push_str(
            &std::iter::repeat_n("?", task_ids.len())
                .collect::<Vec<_>>()
                .join(","),
        );
        sql.push(')');
    }
    sql.push_str(" ORDER BY task_id");

    let mut query = sqlx::query_as::<_, UserTask>(&sql)
        .bind(user_id)
        .bind(type_id);
    if let Some(min_type_id) = min_type_id {
        query = query.bind(min_type_id);
    }
    if let Some(activity_id) = activity_id {
        query = query.bind(activity_id);
    }
    for task_id in task_ids {
        query = query.bind(task_id);
    }
    let tasks = query.fetch_all(pool).await?;
    Ok(tasks
        .into_iter()
        .filter(|task| task.finish_count < max_finish_count(task.type_id, task.task_id))
        .collect())
}

async fn reset_task_ids(
    pool: &SqlitePool,
    user_id: i64,
    groups: Vec<(i32, Vec<i32>)>,
) -> sqlx::Result<()> {
    let now = ServerTime::now_ms();
    for (type_id, task_ids) in groups {
        for task_id in task_ids {
            sqlx::query(
                "UPDATE user_tasks
                 SET progress = 0, has_finished = 0, finish_count = 0, updated_at = ?
                 WHERE user_id = ? AND type_id = ? AND task_id = ?",
            )
            .bind(now)
            .bind(user_id)
            .bind(type_id)
            .bind(task_id)
            .execute(pool)
            .await?;
        }
    }

    Ok(())
}

fn max_finish_count(type_id: i32, task_id: i32) -> i32 {
    let tables = config::configs::get();
    match TaskType::from_id(type_id) {
        Some(TaskType::Daily) => tables
            .task_daily
            .get(task_id)
            .map(|task| task.max_finish_count)
            .unwrap_or(1),
        Some(TaskType::Weekly) => tables
            .task_weekly
            .get(task_id)
            .map(|task| task.max_finish_count)
            .unwrap_or(1),
        Some(TaskType::Achievement) => tables
            .task_achievement
            .get(task_id)
            .map(|task| task.max_finish_count)
            .unwrap_or(1),
        Some(TaskType::Novice) => tables
            .task_guide
            .get(task_id)
            .map(|task| task.max_finish_count)
            .unwrap_or(1),
        Some(TaskType::Room) => tables
            .task_room
            .get(task_id)
            .map(|task| task.max_finish_count)
            .unwrap_or(1),
        Some(TaskType::WeekWalk) => tables
            .task_weekwalk
            .get(task_id)
            .map(|task| task.max_finish_count)
            .unwrap_or(1),
        Some(TaskType::Season) => tables
            .task_season
            .get(task_id)
            .map(|task| task.max_finish_count)
            .unwrap_or(1),
        Some(TaskType::BattlePass) => 1,
        Some(TaskType::Activity106) => tables
            .activity106_task
            .get(task_id)
            .map(|task| task.max_finish_count)
            .unwrap_or(1),
        Some(TaskType::ActivityDungeon) => tables
            .activity113_task
            .get(task_id)
            .map(|task| task.max_finish_count)
            .unwrap_or(1),
        Some(TaskType::Activity119) => 1,
        Some(TaskType::ActivityShow) => tables
            .task_activity_show
            .get(task_id)
            .map(|task| task.max_finish_count)
            .unwrap_or(1),
        Some(TaskType::Activity125) => 1,
        Some(TaskType::Activity128) => tables
            .activity128_task
            .get(task_id)
            .map(|task| task.max_finish_count)
            .unwrap_or(1),
        Some(TaskType::Activity180) => 1,
        Some(TaskType::Activity189) => 1,
        Some(TaskType::Activity194) => 1,
        Some(TaskType::AssassinOutside) => tables
            .assassin_task
            .get(task_id)
            .map(|task| task.max_finish_count)
            .unwrap_or(1),
        Some(TaskType::Odyssey) => 1,
        Some(TaskType::Activity210) => 1,
        Some(TaskType::BpOperAct) => 1,
        Some(TaskType::ActBp) => 1,
        Some(TaskType::Activity220) => 1,
        Some(TaskType::MiniParty) => 1,
        Some(TaskType::ObserverBox) => 1,
        Some(TaskType::Turnback) => 1,
        Some(TaskType::NecrologistStory) => 1,
        None => 1,
    }
}

fn parse_time(value: &str) -> i32 {
    ServerTime::config_datetime_sec(value).unwrap_or(0)
}

struct NewTask {
    user_id: i64,
    task_type_id: i32,
    task_id: i32,
    expiry_time: i32,
    min_type_id: i32,
    activity_id: i32,
}

impl NewTask {
    const fn new(
        user_id: i64,
        task_type_id: i32,
        task_id: i32,
        activity_id: i32,
        min_type_id: i32,
    ) -> Self {
        Self {
            user_id,
            task_type_id,
            task_id,
            expiry_time: 0,
            min_type_id,
            activity_id,
        }
    }
}

struct ConfigTask {
    task_id: i32,
    is_online: bool,
    activity_id: i32,
    min_type_id: i32,
    expiry_time: i32,
}

impl ConfigTask {
    const fn online(task_id: i32, is_online: i32, activity_id: i32) -> Self {
        Self {
            task_id,
            is_online: is_online != 0,
            activity_id,
            min_type_id: 0,
            expiry_time: 0,
        }
    }

    fn looped(task_id: i32, is_online: i32, activity_id: i32, loop_type: i32) -> Self {
        Self {
            task_id,
            is_online: is_online != 0,
            activity_id,
            min_type_id: TaskLoopType::from_id(loop_type)
                .map(TaskLoopType::id)
                .unwrap_or(loop_type),
            expiry_time: 0,
        }
    }

    fn act_bp(
        task_id: i32,
        is_online: i32,
        activity_id: i32,
        loop_type: i32,
        weekly_expiry: i32,
    ) -> Self {
        let min_type_id = TaskLoopType::from_id(loop_type)
            .map(TaskLoopType::id)
            .unwrap_or(loop_type);
        Self {
            task_id,
            is_online: is_online != 0,
            activity_id,
            min_type_id,
            expiry_time: if min_type_id == TaskLoopType::Weekly.id() {
                weekly_expiry
            } else {
                0
            },
        }
    }
}

async fn ensure_config_tasks(
    conn: &mut SqliteConnection,
    user_id: i64,
    type_id: i32,
    tasks: impl IntoIterator<Item = ConfigTask>,
) -> sqlx::Result<()> {
    let tasks = tasks.into_iter().collect::<Vec<_>>();
    for task in tasks.into_iter().filter(|task| task.is_online) {
        ensure_task(
            conn,
            NewTask {
                user_id,
                task_type_id: type_id,
                task_id: task.task_id,
                expiry_time: task.expiry_time,
                min_type_id: task.min_type_id,
                activity_id: task.activity_id,
            },
        )
        .await?;
    }

    Ok(())
}

pub fn current_battle_pass_id() -> Option<i32> {
    current_battle_pass_id_at(ServerTime::now_sec_i32())
}

fn current_battle_pass_id_at(now_sec: i32) -> Option<i32> {
    let tables = config::configs::get();
    pick_current_battle_pass_id(tables.bp.iter().map(|bp| {
        (
            bp.bp_id,
            bp.activity_id,
            tables.battle_pass_bonuses(bp.bp_id).next().is_some(),
            tables
                .battle_pass_tasks(bp.bp_id)
                .filter(|task| task.is_online != 0)
                .any(|task| {
                    let Some(start) = ServerTime::config_datetime_sec(&task.start_time) else {
                        return false;
                    };
                    let Some(end) = ServerTime::config_datetime_sec(&task.end_time) else {
                        return false;
                    };
                    start <= now_sec && now_sec <= end
                }),
        )
    }))
}

fn pick_current_battle_pass_id(
    candidates: impl IntoIterator<Item = (i32, i32, bool, bool)>,
) -> Option<i32> {
    candidates
        .into_iter()
        .filter(|(_, _, has_bonus, is_active)| *has_bonus && *is_active)
        .max_by_key(|(bp_id, activity_id, _, _)| (*activity_id > 0, *activity_id, *bp_id))
        .map(|(bp_id, _, _, _)| bp_id)
}

pub fn current_battle_pass() -> Option<&'static config::bp::Bp> {
    let bp_id = current_battle_pass_id()?;
    config::configs::get().battle_pass(bp_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_pool() -> SqlitePool {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (1, 'act233-tasks', 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn act_bp_tasks_use_loop_order_and_loop_owned_expiry() {
        let pool = test_pool().await;
        ensure_tasks_for_type(&pool, 1, TaskType::ActBp)
            .await
            .unwrap();

        let tasks = list_act_bp(&pool, 1, 13716).await.unwrap();
        assert_eq!(tasks.len(), 15);
        assert!(tasks.windows(2).all(|pair| {
            (pair[0].min_type_id, pair[0].task_id) <= (pair[1].min_type_id, pair[1].task_id)
        }));
        assert_eq!(
            tasks
                .iter()
                .filter(|task| task.min_type_id == TaskLoopType::Weekly.id())
                .count(),
            8
        );
        assert_eq!(
            tasks
                .iter()
                .filter(|task| task.min_type_id == TaskLoopType::Permanent.id())
                .count(),
            7
        );
        let weekly_expiry = ServerTime::next_weekly_refresh_sec(ServerTime::now_ms());
        assert!(
            tasks
                .iter()
                .filter(|task| task.min_type_id == TaskLoopType::Weekly.id())
                .all(|task| task.expiry_time == weekly_expiry)
        );
        assert!(
            tasks
                .iter()
                .filter(|task| task.min_type_id == TaskLoopType::Permanent.id())
                .all(|task| task.expiry_time == 0)
        );

        sync_login_tasks(&pool, 1, false).await.unwrap();
        let refreshed = list_act_bp(&pool, 1, 13716).await.unwrap();
        assert!(
            refreshed
                .iter()
                .filter(|task| task.min_type_id == 2)
                .all(|task| task.expiry_time == weekly_expiry)
        );
        assert!(
            refreshed
                .iter()
                .filter(|task| task.min_type_id == 3)
                .all(|task| task.expiry_time == 0)
        );

        set_progress(&pool, 1, TaskType::ActBp.id(), 790001, 1, true)
            .await
            .unwrap();
        set_progress(&pool, 1, TaskType::ActBp.id(), 790009, 5, false)
            .await
            .unwrap();
        reset_weekly_tasks(&pool, 1).await.unwrap();

        let reset = list_act_bp(&pool, 1, 13716).await.unwrap();
        let weekly = reset.iter().find(|task| task.task_id == 790001).unwrap();
        assert_eq!(weekly.progress, 0);
        assert!(!weekly.has_finished);
        let permanent = reset.iter().find(|task| task.task_id == 790009).unwrap();
        assert_eq!(permanent.progress, 5);
    }

    #[tokio::test]
    async fn act_bp_login_days_progression_uses_online_login_config() {
        let pool = test_pool().await;

        let updated = sync_login_tasks(&pool, 1, true).await.unwrap();
        let first = updated.iter().find(|task| task.task_id == 790001).unwrap();
        assert_eq!(first.type_id, TaskType::ActBp.id());
        assert_eq!(first.progress, 1);
        assert!(first.has_finished);
        assert!(!updated.iter().any(|task| task.task_id == 790005));
    }

    #[test]
    fn battle_pass_selection_follows_dated_online_task_windows() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);

        let before_rollover = ServerTime::config_datetime_sec("2026-08-13 04:59:59").unwrap();
        let at_rollover = ServerTime::config_datetime_sec("2026-08-13 05:00:00").unwrap();

        assert_eq!(current_battle_pass_id_at(before_rollover), Some(28));
        assert_eq!(current_battle_pass_id_at(at_rollover), Some(26));
    }

    #[test]
    fn battle_pass_selection_excludes_blank_only_windows() {
        let candidates = [(26, 13_736, true, true), (28, 138_512, true, false)];

        assert_eq!(pick_current_battle_pass_id(candidates), Some(26));
    }
}
