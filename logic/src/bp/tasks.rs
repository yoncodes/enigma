use super::*;

pub(super) async fn task_red_dot_infos(
    db: &SqlitePool,
    player_id: i64,
) -> Result<Vec<RedDotInfo>, AppError> {
    let Some(bp_id) = task_db::current_battle_pass_id() else {
        return Ok(Vec::new());
    };

    task_red_dot_infos_for(db, player_id, bp_id).await
}

pub(super) async fn task_red_dot_infos_for(
    db: &SqlitePool,
    player_id: i64,
    bp_id: i32,
) -> Result<Vec<RedDotInfo>, AppError> {
    let state = battle_pass::get_or_create_state(db, player_id, bp_id).await?;
    task_db::ensure_battle_pass_tasks(db, player_id, bp_id).await?;
    task_db::ensure_bp_oper_act_tasks(db, player_id, bp_id).await?;

    let tasks = task_db::list_battle_pass(db, player_id, bp_id).await?;
    let week_score_full = is_week_score_full(bp_id, state.weekly_score);
    let mut counts = HashMap::<i64, i32>::new();

    for task in tasks {
        let Some((loop_type, max_progress)) = bp_task_red_dot_config(&task, bp_id) else {
            continue;
        };

        if task.progress >= max_progress
            && task.finish_count == 0
            && should_show_task_red_dot(loop_type, week_score_full)
        {
            *counts.entry(task_tab_id(loop_type)).or_default() += 1;
        }
    }

    Ok(counts
        .into_iter()
        .map(|(id, value)| RedDotInfo {
            id,
            value,
            time: Some(0),
            ext: None,
        })
        .collect())
}

pub fn task_score_from_tasks(bp_id: i32, tasks: &[Task]) -> i32 {
    task_score(
        bp_id,
        tasks
            .iter()
            .map(|task| (task.id, task.r#type.unwrap_or_default())),
    )
}

pub(crate) fn task_score_from_models(
    bp_id: i32,
    tasks: &[database::models::game::tasks::UserTask],
) -> i32 {
    task_score(bp_id, tasks.iter().map(|task| (task.task_id, task.type_id)))
}

fn task_score(bp_id: i32, tasks: impl IntoIterator<Item = (i32, i32)>) -> i32 {
    let tables = config::configs::get();
    tasks
        .into_iter()
        .map(|(task_id, type_id)| match type_id {
            type_id if type_id == task_db::TaskType::BattlePass.id() => tables
                .bp_task
                .get(task_id)
                .filter(|config| config.bp_id == bp_id)
                .map_or(0, |config| config.bonus_score),
            type_id if type_id == task_db::TaskType::BpOperAct.id() => tables
                .activity214_task
                .get(task_id)
                .filter(|config| config.bp_id == bp_id)
                .map_or(0, |config| config.bonus_score),
            _ => 0,
        })
        .sum()
}

pub fn has_task_red_dot(tasks: &[Task]) -> bool {
    tasks.iter().any(|task| {
        task.r#type == Some(task_db::TaskType::BattlePass.id())
            || task.r#type == Some(task_db::TaskType::BpOperAct.id())
    })
}

fn bp_task_red_dot_config(
    task: &database::models::game::tasks::UserTask,
    bp_id: i32,
) -> Option<(TaskLoopType, i32)> {
    let tables = config::configs::get();
    match task_db::TaskType::from_id(task.type_id) {
        Some(task_db::TaskType::BattlePass) => tables
            .bp_task
            .get(task.task_id)
            .filter(|config| config.bp_id == bp_id)
            .map(|config| (config.loop_type, config.max_progress)),
        Some(task_db::TaskType::BpOperAct) => tables
            .activity214_task
            .get(task.task_id)
            .filter(|config| config.bp_id == bp_id)
            .map(|config| (config.loop_type, config.max_progress)),
        _ => None,
    }
    .map(|(loop_type, max_progress)| {
        (
            TaskLoopType::from_id(loop_type).unwrap_or(TaskLoopType::Permanent),
            max_progress,
        )
    })
}

pub(super) fn task_tab_id(loop_type: TaskLoopType) -> i64 {
    match loop_type {
        TaskLoopType::Appoint => TaskLoopType::Permanent.id() as i64,
        other => other.id() as i64,
    }
}

pub(super) fn should_show_task_red_dot(loop_type: TaskLoopType, week_score_full: bool) -> bool {
    !week_score_full || task_tab_id(loop_type) == TaskLoopType::Permanent.id() as i64
}

fn is_week_score_full(bp_id: i32, weekly_score: i32) -> bool {
    let weekly_max = weekly_max_score(bp_id);

    weekly_score >= weekly_max
}

fn weekly_max_score(bp_id: i32) -> i32 {
    let tables = config::configs::get();
    let base = tables
        .r#const
        .get(112)
        .and_then(|row| row.value.parse::<i32>().ok())
        .unwrap_or(10_000);
    let Some(bp) = tables.battle_pass(bp_id) else {
        return base;
    };

    let rate = 1000 + bp.week_limit_times.max(0);
    if rate > 1000 {
        rate * base / 1000
    } else {
        base
    }
}

pub(super) fn score_bonus_info(
    bp_id: i32,
    state: Option<&battle_pass::BattlePassState>,
) -> Vec<BpScoreBonusInfo> {
    config::configs::get()
        .battle_pass_bonuses(bp_id)
        .map(|bonus| BpScoreBonusInfo {
            level: Some(bonus.level),
            has_getfree_bonus: Some(
                state.is_some_and(|state| state.has_get_free_bonus.contains(&bonus.level)),
            ),
            has_get_pay_bonus: Some(
                state.is_some_and(|state| state.has_get_pay_bonus.contains(&bonus.level)),
            ),
            has_get_spfree_bonus: Some(
                state.is_some_and(|state| state.has_get_sp_free_bonus.contains(&bonus.level)),
            ),
            has_get_sp_pay_bonus: Some(
                state.is_some_and(|state| state.has_get_sp_pay_bonus.contains(&bonus.level)),
            ),
        })
        .collect()
}

pub(super) fn parse_bp_reward(reward_value: &str, owned_skins: &[i32]) -> reward::RewardSet {
    let mut rewards = reward::parse(reward_value);
    rewards
        .skins
        .retain(|(skin_id, _)| !owned_skins.contains(skin_id));
    rewards
}

pub(super) fn bp_time_range(bp_id: i32) -> (Option<i32>, Option<i32>) {
    config::configs::get()
        .battle_pass_tasks(bp_id)
        .filter_map(|task| {
            Some((
                ServerTime::config_datetime_sec(&task.start_time)?,
                ServerTime::config_datetime_sec(&task.end_time)?,
            ))
        })
        .fold((None, None), |(start, end), (task_start, task_end)| {
            (
                Some(start.map_or(task_start, |value: i32| value.min(task_start))),
                Some(end.map_or(task_end, |value: i32| value.max(task_end))),
            )
        })
}
