use crate::models::game::tasks::UserTask;
use common::time::ServerTime;
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::db::game::battle_pass;

use super::{
    TaskType, add_progress_in_transaction, current_battle_pass_id,
    ensure_tasks_for_type_in_transaction,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskEvent {
    Act222ArcadeSettle {
        activity_id: i32,
    },
    Act205FinishGame {
        activity_id: i32,
        game_type: i32,
        is_win: bool,
    },
    CurrencyDec {
        currency_id: i32,
        amount: i32,
    },
    Summon {
        pool_id: i32,
        count: i32,
    },
    DoneCount {
        name: &'static str,
        count: i32,
    },
    DungeonPass {
        chapter_type: i32,
        count: i32,
    },
    EpisodeFinish {
        episode_id: i32,
    },
    HeroLevelReach {
        hero_id: i32,
        level: i32,
    },
    ProductionLine {
        action: ProductionLineAction,
        count: i32,
    },
    StoreGoodsBought {
        store_id: i32,
    },
    TaskFinish {
        task_id: i32,
    },
    TowerMopUp {
        count: i32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionLineAction {
    Gather = 1,
    Create = 2,
}

impl TaskEvent {
    pub fn hero_touch_count(self) -> Option<i32> {
        match self {
            Self::DoneCount {
                name: "HeroTouch",
                count,
            } => Some(count),
            _ => None,
        }
    }

    fn count(self) -> i32 {
        match self {
            Self::Act222ArcadeSettle { .. } | Self::Act205FinishGame { .. } => 1,
            Self::CurrencyDec { amount, .. } => amount.max(1),
            Self::Summon { count, .. } => count.max(1),
            Self::DoneCount { count, .. }
            | Self::DungeonPass { count, .. }
            | Self::ProductionLine { count, .. }
            | Self::TowerMopUp { count } => count.max(1),
            Self::EpisodeFinish { .. }
            | Self::HeroLevelReach { .. }
            | Self::StoreGoodsBought { .. }
            | Self::TaskFinish { .. } => 1,
        }
    }

    fn matches(self, listener_type: &str, listener_param: &str) -> bool {
        match self {
            Self::Act222ArcadeSettle { activity_id } => {
                listener_type == "Act222ArcadeSettle"
                    && listener_param.parse::<i32>() == Ok(activity_id)
            }
            Self::Act205FinishGame {
                activity_id,
                game_type,
                is_win,
            } => {
                (listener_type == "Act205FinishGame"
                    || (is_win && listener_type == "Act205GameWin"))
                    && matches_pair_param(listener_param, activity_id, game_type)
            }
            Self::CurrencyDec { currency_id, .. } => {
                listener_type == "CurrencyDec"
                    && listener_param
                        .parse::<i32>()
                        .is_ok_and(|param| param == currency_id)
            }
            Self::Summon { pool_id, .. } => {
                (listener_type == "Summon"
                    && listener_param
                        .parse::<i32>()
                        .is_ok_and(|param| param == pool_id))
                    || (listener_type == "DoneCount" && listener_param == "Summon")
            }
            Self::DoneCount { name, .. } => listener_type == "DoneCount" && listener_param == name,
            Self::DungeonPass { chapter_type, .. } => {
                listener_type == "DungeonPass" && contains_i32_param(listener_param, chapter_type)
            }
            Self::EpisodeFinish { episode_id } => {
                listener_type == "EpisodeFinish"
                    && listener_param
                        .parse::<i32>()
                        .is_ok_and(|param| param == episode_id)
            }
            Self::HeroLevelReach { hero_id, level } => {
                (listener_type == "HeroLevelReach"
                    && matches_hero_level_reach_param(listener_param, hero_id, level))
                    || (listener_type == "DoneCount" && listener_param == "HeroLevelUp")
            }
            Self::ProductionLine { action, .. } => {
                listener_type == "ProductionLine"
                    && listener_param
                        .parse::<i32>()
                        .is_ok_and(|param| param == action as i32)
            }
            Self::StoreGoodsBought { store_id } => {
                listener_type == "BuyStoreGoods" && listener_param.parse::<i32>() == Ok(store_id)
            }
            Self::TaskFinish { task_id } => {
                listener_type == "TaskFinish"
                    && listener_param
                        .strip_prefix("taskIds=")
                        .is_some_and(|ids| contains_i32_param(ids, task_id))
            }
            Self::TowerMopUp { .. } => listener_type == "TowerMopUpTime",
        }
    }

    pub(crate) fn achievement_increment(
        self,
        listener_type: &str,
        listener_param: &str,
    ) -> Option<i32> {
        match self {
            Self::CurrencyDec {
                currency_id,
                amount,
            } if listener_type == "CurrencyDecTotal"
                && listener_param.parse::<i32>() == Ok(currency_id) =>
            {
                Some(amount.max(1))
            }
            Self::DoneCount { name, count }
                if listener_type == "DoneCountTotal" && listener_param == name =>
            {
                Some(count.max(1))
            }
            Self::Summon { count, .. }
                if listener_type == "DoneCountTotal" && listener_param == "Summon" =>
            {
                Some(count.max(1))
            }
            Self::HeroLevelReach { .. }
                if listener_type == "DoneCountTotal" && listener_param == "HeroLevelUp" =>
            {
                Some(1)
            }
            _ => None,
        }
    }
}

pub async fn sync_event_tasks(
    pool: &SqlitePool,
    user_id: i64,
    event: TaskEvent,
) -> sqlx::Result<Vec<UserTask>> {
    let mut tx = pool.begin().await?;
    let updated = sync_event_tasks_in_transaction(&mut tx, user_id, event).await?;
    tx.commit().await?;
    Ok(updated)
}

pub async fn sync_event_tasks_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    event: TaskEvent,
) -> sqlx::Result<Vec<UserTask>> {
    ensure_tasks_for_type_in_transaction(tx, user_id, TaskType::Daily).await?;
    ensure_tasks_for_type_in_transaction(tx, user_id, TaskType::Weekly).await?;
    ensure_tasks_for_type_in_transaction(tx, user_id, TaskType::BattlePass).await?;
    ensure_tasks_for_type_in_transaction(tx, user_id, TaskType::BpOperAct).await?;
    ensure_tasks_for_type_in_transaction(tx, user_id, TaskType::ActBp).await?;
    ensure_tasks_for_type_in_transaction(tx, user_id, TaskType::VersionActivity).await?;
    ensure_tasks_for_type_in_transaction(tx, user_id, TaskType::Activity125).await?;

    let bp_id = current_battle_pass_id();
    let include_bp = match bp_id {
        Some(bp_id) => !battle_pass::score_maxed_in_transaction(tx, user_id, bp_id).await?,
        None => false,
    };
    let mut updated = Vec::new();
    for target in event_task_targets(event, bp_id.filter(|_| include_bp)) {
        if let Some(task) = add_progress_in_transaction(
            tx,
            user_id,
            target.type_id,
            target.task_id,
            event.count(),
            target.max_progress,
        )
        .await?
        {
            updated.push(task);
        }
    }

    Ok(updated)
}

pub async fn sync_hero_invitation_claims_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
) -> sqlx::Result<Vec<UserTask>> {
    ensure_tasks_for_type_in_transaction(tx, user_id, TaskType::ActivityDungeon).await?;
    let claim_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM user_hero_invitation_claims
         WHERE user_id = ? AND invite_id > 0",
    )
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await?
    .min(i64::from(i32::MAX)) as i32;
    let now = ServerTime::now_ms();
    let mut updated = Vec::new();

    for target in config::configs::get()
        .activity113_task
        .iter()
        .filter(|task| {
            task.is_online != 0
                && task.listener_type == "GainInviteRewardCount"
                && task.listener_param.is_empty()
        })
    {
        let Some(task) = sqlx::query_as::<_, UserTask>(
            "SELECT user_id, type_id, task_id, progress, has_finished, finish_count,
                    expiry_time, min_type_id, activity_id, created_at, updated_at
             FROM user_tasks
             WHERE user_id = ? AND type_id = ? AND task_id = ?",
        )
        .bind(user_id)
        .bind(TaskType::ActivityDungeon.id())
        .bind(target.id)
        .fetch_optional(&mut **tx)
        .await?
        else {
            continue;
        };

        let max_progress = target.max_progress.max(1);
        let progress = task.progress.max(claim_count.min(max_progress));
        let has_finished = progress >= max_progress && task.finish_count < target.max_finish_count;
        if progress == task.progress && has_finished == task.has_finished {
            continue;
        }

        sqlx::query(
            "UPDATE user_tasks
             SET progress = ?, has_finished = ?, updated_at = ?
             WHERE user_id = ? AND type_id = ? AND task_id = ?",
        )
        .bind(progress)
        .bind(has_finished)
        .bind(now)
        .bind(user_id)
        .bind(TaskType::ActivityDungeon.id())
        .bind(target.id)
        .execute(&mut **tx)
        .await?;

        updated.push(
            sqlx::query_as::<_, UserTask>(
                "SELECT user_id, type_id, task_id, progress, has_finished, finish_count,
                        expiry_time, min_type_id, activity_id, created_at, updated_at
                 FROM user_tasks
                 WHERE user_id = ? AND type_id = ? AND task_id = ?",
            )
            .bind(user_id)
            .bind(TaskType::ActivityDungeon.id())
            .bind(target.id)
            .fetch_one(&mut **tx)
            .await?,
        );
    }

    Ok(updated)
}

fn event_task_targets(event: TaskEvent, bp_id: Option<i32>) -> Vec<TaskTarget> {
    let tables = config::configs::get();
    let mut targets = Vec::new();

    targets.extend(
        tables
            .online_daily_tasks()
            .filter(|task| event.matches(&task.listener_type, &task.listener_param))
            .map(|task| TaskTarget::new(TaskType::Daily, task.id, task.max_progress)),
    );
    targets.extend(
        tables
            .online_weekly_tasks()
            .filter(|task| event.matches(&task.listener_type, &task.listener_param))
            .map(|task| TaskTarget::new(TaskType::Weekly, task.id, task.max_progress)),
    );

    if let Some(bp_id) = bp_id {
        targets.extend(
            tables
                .battle_pass_tasks(bp_id)
                .filter(|task| {
                    task.is_online != 0 && event.matches(&task.listener_type, &task.listener_param)
                })
                .map(|task| TaskTarget::new(TaskType::BattlePass, task.id, task.max_progress)),
        );
        targets.extend(
            tables
                .activity214_task
                .iter()
                .filter(|task| {
                    task.bp_id == bp_id
                        && task.is_online != 0
                        && event.matches(&task.listener_type, &task.listener_param)
                })
                .map(|task| TaskTarget::new(TaskType::BpOperAct, task.id, task.max_progress)),
        );
    }
    targets.extend(
        tables
            .turnback_task
            .iter()
            .filter(|task| {
                task.is_online != 0 && event.matches(&task.listener_type, &task.listener_param)
            })
            .map(|task| TaskTarget::new(TaskType::Turnback, task.id, task.max_progress)),
    );
    targets.extend(
        tables
            .activity125_task
            .iter()
            .filter(|task| {
                task.is_online != 0 && event.matches(&task.listener_type, &task.listener_param)
            })
            .map(|task| TaskTarget::new(TaskType::Activity125, task.id, task.max_progress)),
    );
    targets.extend(
        tables
            .copost_version_task
            .iter()
            .filter(|task| {
                task.version_id
                    == tables
                        .copost_version_task
                        .iter()
                        .map(|row| row.version_id)
                        .max()
                        .unwrap_or_default()
                    && event.matches(&task.listener_type, &task.listener_param)
            })
            .map(|task| TaskTarget::new(TaskType::VersionActivity, task.id, task.max_progress)),
    );
    targets.extend(
        tables
            .activity233_task
            .iter()
            .filter(|task| {
                task.is_online != 0 && event.matches(&task.listener_type, &task.listener_param)
            })
            .map(|task| TaskTarget::new(TaskType::ActBp, task.id, task.max_progress)),
    );

    targets
}

fn contains_i32_param(value: &str, expected: i32) -> bool {
    value
        .split('#')
        .filter_map(|part| part.parse::<i32>().ok())
        .any(|param| param == expected)
}

fn matches_pair_param(value: &str, first: i32, second: i32) -> bool {
    let mut parts = value.split('#').filter_map(|part| part.parse::<i32>().ok());
    parts.next() == Some(first) && parts.next() == Some(second)
}

fn matches_hero_level_reach_param(value: &str, hero_id: i32, level: i32) -> bool {
    let mut parts = value.split(',');
    let configured_hero = parts
        .next()
        .and_then(|part| part.strip_prefix("heroId="))
        .and_then(|part| part.parse::<i32>().ok());
    let configured_level = parts
        .next()
        .and_then(|part| part.strip_prefix("level="))
        .and_then(|part| part.parse::<i32>().ok());

    configured_hero == Some(hero_id) && configured_level.is_some_and(|required| level >= required)
}

struct TaskTarget {
    type_id: i32,
    task_id: i32,
    max_progress: i32,
}

impl TaskTarget {
    fn new(task_type: TaskType, task_id: i32, max_progress: i32) -> Self {
        Self {
            type_id: task_type.id(),
            task_id,
            max_progress,
        }
    }
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
             VALUES (1, 'act233-events', 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[test]
    fn tower_mop_up_routes_to_current_pass_and_returner_tasks() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let targets =
            event_task_targets(TaskEvent::TowerMopUp { count: 4 }, current_battle_pass_id());

        assert!(
            targets
                .iter()
                .any(|target| target.type_id == TaskType::BpOperAct.id())
        );
        assert!(
            targets
                .iter()
                .any(|target| target.type_id == TaskType::Turnback.id())
        );
    }

    #[test]
    fn production_line_actions_keep_gather_and_create_tasks_distinct() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);

        let gather = event_task_targets(
            TaskEvent::ProductionLine {
                action: ProductionLineAction::Gather,
                count: 3,
            },
            None,
        );
        let create = event_task_targets(
            TaskEvent::ProductionLine {
                action: ProductionLineAction::Create,
                count: 1,
            },
            None,
        );

        assert!(gather.iter().any(|target| target.task_id == 10061));
        assert!(!gather.iter().any(|target| target.task_id == 20101));
        assert!(create.iter().any(|target| target.task_id == 20101));
        assert!(!create.iter().any(|target| target.task_id == 10061));
    }

    #[test]
    fn store_purchase_routes_by_store_id() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);

        let targets = event_task_targets(TaskEvent::StoreGoodsBought { store_id: 111 }, None);
        assert!(targets.iter().any(|target| target.task_id == 180054));
        assert!(
            event_task_targets(TaskEvent::StoreGoodsBought { store_id: 614 }, None)
                .iter()
                .all(|target| target.task_id != 180054)
        );
    }

    #[test]
    fn activity233_routes_only_exact_supported_listeners() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);

        let currency = event_task_targets(
            TaskEvent::CurrencyDec {
                currency_id: 4,
                amount: 1,
            },
            None,
        );
        assert!(currency.iter().any(|target| target.task_id == 790005));
        assert!(!currency.iter().any(|target| target.task_id == 790007));

        let summon = event_task_targets(
            TaskEvent::Summon {
                pool_id: 37111,
                count: 1,
            },
            None,
        );
        assert!(summon.iter().any(|target| target.task_id == 790009));
        assert!(
            !summon.iter().any(|target| {
                target.type_id == TaskType::ActBp.id() && target.task_id >= 790012
            })
        );
        assert!(
            !event_task_targets(
                TaskEvent::Summon {
                    pool_id: 37112,
                    count: 1,
                },
                None,
            )
            .iter()
            .any(|target| target.task_id == 790009)
        );

        let hero_level = event_task_targets(
            TaskEvent::HeroLevelReach {
                hero_id: 3144,
                level: 30,
            },
            None,
        );
        assert!(hero_level.iter().any(|target| target.task_id == 790010));
        assert!(
            event_task_targets(
                TaskEvent::HeroLevelReach {
                    hero_id: 3144,
                    level: 31,
                },
                None,
            )
            .iter()
            .any(|target| target.task_id == 790010)
        );
        assert!(
            !event_task_targets(
                TaskEvent::HeroLevelReach {
                    hero_id: 3144,
                    level: 29,
                },
                None,
            )
            .iter()
            .any(|target| target.task_id == 790010)
        );
    }

    #[test]
    fn arcade_win_targets_version_activity_and_captured_pass_tasks() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let updated = event_task_targets(
            TaskEvent::Act222ArcadeSettle { activity_id: 13720 },
            Some(26),
        );

        assert!(
            updated.iter().any(|task| {
                task.type_id == TaskType::VersionActivity.id() && task.task_id == 912
            })
        );
        assert!(
            updated
                .iter()
                .any(|task| { task.type_id == TaskType::ActBp.id() && task.task_id == 790012 })
        );
        assert!(
            updated.iter().any(|task| {
                task.type_id == TaskType::BattlePass.id() && task.task_id == 103730
            })
        );
    }

    #[test]
    fn generic_events_do_not_route_activity_dungeon_tasks() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let targets = event_task_targets(
            TaskEvent::CurrencyDec {
                currency_id: 3,
                amount: 1,
            },
            None,
        );
        assert!(
            targets
                .iter()
                .all(|target| target.type_id != TaskType::ActivityDungeon.id())
        );
    }

    #[tokio::test]
    async fn activity233_event_progression_updates_type_79_tasks() {
        let pool = test_pool().await;

        let currency = sync_event_tasks(
            &pool,
            1,
            TaskEvent::CurrencyDec {
                currency_id: 4,
                amount: 500,
            },
        )
        .await
        .unwrap();
        let currency_task = currency.iter().find(|task| task.task_id == 790005).unwrap();
        assert_eq!(currency_task.progress, 500);
        assert!(currency_task.has_finished);

        let summon = sync_event_tasks(
            &pool,
            1,
            TaskEvent::Summon {
                pool_id: 37111,
                count: 1,
            },
        )
        .await
        .unwrap();
        let summon_task = summon.iter().find(|task| task.task_id == 790009).unwrap();
        assert_eq!(summon_task.progress, 1);
        assert_eq!(summon_task.type_id, TaskType::ActBp.id());
    }

    #[tokio::test]
    async fn hero_invitation_claim_sync_updates_only_its_activity_dungeon_tasks() {
        let pool = test_pool().await;
        let mut tx = pool.begin().await.unwrap();
        sqlx::query(
            "INSERT INTO user_hero_invitation_claims (user_id, invite_id, claimed_at)
             VALUES (1, 1, 0)",
        )
        .execute(&mut *tx)
        .await
        .unwrap();
        let updated = sync_hero_invitation_claims_in_transaction(&mut tx, 1)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let mut ids = updated.iter().map(|task| task.task_id).collect::<Vec<_>>();
        ids.sort_unstable();
        assert_eq!(ids, vec![110918, 110919, 110920]);
        assert!(updated.iter().all(|task| {
            task.type_id == TaskType::ActivityDungeon.id()
                && task.progress == 1
                && !task.has_finished
        }));
    }
}
