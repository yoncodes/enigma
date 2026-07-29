use crate::{error::AppError, reward};
use database::{db::game::tasks as task_db, models::game::tasks::UserTaskActivity};

pub(super) async fn add_claim_activity_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    player_id: i64,
    task: &database::models::game::tasks::UserTask,
) -> Result<(Vec<UserTaskActivity>, reward::RewardSet), AppError> {
    let Some(mut activity) = task_db::add_activity_in_transaction(
        tx,
        player_id,
        task.type_id,
        task_activity_value(task.type_id, task.task_id),
        task.expiry_time,
    )
    .await?
    else {
        return Ok((Vec::new(), reward::RewardSet::default()));
    };
    let mut rewards = reward::RewardSet::default();
    if !matches!(
        task_db::TaskType::from_id(task.type_id),
        Some(task_db::TaskType::Daily | task_db::TaskType::Weekly)
    ) {
        return Ok((vec![activity], rewards));
    }

    loop {
        let next_define_id = activity.define_id + 1;
        let Some(bonus) = task_activity_bonus(task.type_id, next_define_id) else {
            break;
        };
        let (updated, claimed) = task_db::claim_activity_bonus_in_transaction(
            tx,
            player_id,
            task.type_id,
            next_define_id,
            bonus.need_activity,
        )
        .await?;
        activity = updated;
        if !claimed {
            break;
        }
        rewards.extend(parse_task_reward(&bonus.bonus));
    }

    Ok((vec![activity], rewards))
}

pub(super) fn task_rewards(type_id: i32, task_id: i32) -> reward::RewardSet {
    task_bonus(type_id, task_id)
        .map(|bonus| parse_task_reward(&bonus))
        .unwrap_or_default()
}

fn task_bonus(type_id: i32, task_id: i32) -> Option<String> {
    let tables = config::configs::get();
    match task_db::TaskType::from_id(type_id) {
        Some(task_db::TaskType::Daily) => tables
            .task_daily
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::Weekly) => tables
            .task_weekly
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::Achievement) => tables
            .task_achievement
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::Novice) => tables
            .task_guide
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::Room) => {
            tables.task_room.get(task_id).map(|task| task.bonus.clone())
        }
        Some(task_db::TaskType::WeekWalk) => tables
            .task_weekwalk
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::Activity106) => tables
            .activity106_task
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::Season) => tables
            .task_season
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::ActivityDungeon) => tables
            .activity113_task
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::Activity119) => tables
            .activity119_task
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::ActivityShow) => tables
            .task_activity_show
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::Activity125) => tables
            .activity125_task
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::Activity128) => tables
            .activity128_task
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::Activity180) => tables
            .activity180_task
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::Activity189) => tables
            .activity189_task
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::Activity194) => tables
            .activity194_task
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::AssassinOutside) => tables
            .assassin_task
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::Odyssey) => tables
            .odyssey_task
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::Activity210) => tables
            .activity210_task
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::Activity220) => tables
            .activity220_task
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::MiniParty) => tables
            .activity223_task
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::ObserverBox) => tables
            .activity226_task
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::Turnback) => tables
            .turnback_task
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::NecrologistStory) => tables
            .hero_story_task
            .get(task_id)
            .map(|task| task.bonus.clone()),
        Some(task_db::TaskType::BattlePass) | Some(task_db::TaskType::BpOperAct) | None => None,
    }
}

fn task_activity_value(type_id: i32, task_id: i32) -> i32 {
    let tables = config::configs::get();
    match task_db::TaskType::from_id(type_id) {
        Some(task_db::TaskType::Daily) => tables
            .task_daily
            .get(task_id)
            .map(|task| task.activity)
            .unwrap_or_default(),
        Some(task_db::TaskType::Weekly) => tables
            .task_weekly
            .get(task_id)
            .map(|task| task.activity)
            .unwrap_or_default(),
        Some(task_db::TaskType::Novice) => tables
            .task_guide
            .get(task_id)
            .map(|task| task.activity)
            .unwrap_or_default(),
        Some(task_db::TaskType::ActivityDungeon) => tables
            .activity113_task
            .get(task_id)
            .map(|task| task.activity)
            .unwrap_or_default(),
        Some(task_db::TaskType::Activity128) => tables
            .activity128_task
            .get(task_id)
            .map(|task| task.activity)
            .unwrap_or_default(),
        Some(task_db::TaskType::ActivityShow) => tables
            .task_activity_show
            .get(task_id)
            .map(|task| task.activity)
            .unwrap_or_default(),
        _ => 0,
    }
}

pub(super) fn task_activity_bonus(
    type_id: i32,
    define_id: i32,
) -> Option<&'static config::task_activity_bonus::TaskActivityBonus> {
    config::configs::get()
        .task_activity_bonus
        .iter()
        .find(|bonus| bonus.r#type == type_id && bonus.id == define_id)
}

pub(super) fn parse_task_reward(value: &str) -> reward::RewardSet {
    if value.trim().is_empty() {
        return reward::RewardSet::default();
    }

    if value.contains('#') {
        return reward::parse(value);
    }

    value
        .parse::<i32>()
        .map(reward::parse_reward_id)
        .unwrap_or_default()
}
