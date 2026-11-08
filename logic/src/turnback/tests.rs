use super::TurnbackManager;
use super::claims::{claim_turnback_bonus_ids, coalesced_material_changes};
use crate::{error::AppError, reward};
use database::db::game::{currencies, tasks::TaskType};
use sqlx::SqlitePool;

#[test]
fn double_bonus_materials_are_aggregated_by_type_and_id() {
    let mut rewards = reward::parse("2#2#300|1#481006#3");
    rewards.extend(reward::parse("2#2#60|1#481006#2"));

    assert_eq!(
        coalesced_material_changes(&rewards),
        vec![(1, 481006, 5), (2, 2, 360)]
    );
}

async fn database(player_id: i64) -> SqlitePool {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'turnback-return', 0, 0)",
    )
    .bind(player_id)
    .execute(&db)
    .await
    .unwrap();
    db
}

#[tokio::test]
async fn return_rewards_follow_range_vitality_and_claim_history() {
    let player_id = 73;
    let db = database(player_id).await;
    let tables = config::configs::get();
    let turnback = tables
        .turnback
        .iter()
        .find(|row| !row.resource_return_condition.is_empty())
        .unwrap();
    let range = tables.turnback_return_range.iter().next().unwrap();
    let range_end = range
        .loss_days
        .split_once('|')
        .unwrap()
        .1
        .parse::<i32>()
        .unwrap();
    let now = common::time::ServerTime::now_sec_i32();
    let start_time = now - 2 * 86_400;
    let end_time = now + 12 * 86_400;
    let leave_time = start_time - (range_end - 1) * 86_400;
    sqlx::query(
        "INSERT INTO user_turnback_state
            (user_id, turnback_id, start_time, end_time, leave_time)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(player_id)
    .bind(turnback.id)
    .bind(start_time)
    .bind(end_time)
    .bind(leave_time)
    .execute(&db)
    .await
    .unwrap();

    let currency_id = turnback
        .resource_return_vitality
        .split_once('#')
        .unwrap()
        .1
        .parse::<i32>()
        .unwrap();
    let mut rows = tables
        .turnback_return_reward
        .iter()
        .filter(|row| row.turnback_id == turnback.id && row.loss_days_range == range.range_id)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.need_vitality);
    let vitality = rows[2].need_vitality;
    currencies::add_currency(&db, player_id, currency_id, vitality)
        .await
        .unwrap();

    let manager = TurnbackManager::new(player_id);
    manager.sync_state(&db, tables).await.unwrap();
    let resource = manager
        .info(&db, tables)
        .await
        .unwrap()
        .info
        .unwrap()
        .resource_return
        .unwrap();
    assert_eq!(resource.open, Some(true));
    assert_eq!(resource.loss_days_range_id, Some(range.range_id));
    assert_eq!(resource.vitality, Some(vitality));
    assert_eq!(resource.resource_return_tasks.len(), 2);
    assert!(!resource.left_task_ids.is_empty());
    assert_eq!(resource.rewards[0].state, Some(1));
    assert_eq!(resource.rewards[3].state, Some(0));
    let random_task = resource
        .resource_return_tasks
        .iter()
        .find(|task| {
            tables
                .turnback_task
                .get(task.id)
                .is_some_and(|row| row.turnback_task_type == 2)
        })
        .unwrap()
        .id;
    let fixed_task = resource
        .resource_return_tasks
        .iter()
        .find(|task| task.id != random_task)
        .unwrap()
        .id;
    let left_count = resource.left_task_ids.len();
    sqlx::query(
        "UPDATE user_tasks SET finish_count = 1
         WHERE user_id = ? AND type_id = ? AND task_id = ?",
    )
    .bind(player_id)
    .bind(TaskType::Turnback.id())
    .bind(random_task)
    .execute(&db)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE user_tasks SET progress = 1, has_finished = 0, finish_count = 1
         WHERE user_id = ? AND type_id = ? AND task_id = ?",
    )
    .bind(player_id)
    .bind(TaskType::Turnback.id())
    .bind(fixed_task)
    .execute(&db)
    .await
    .unwrap();
    let rotated = manager
        .info(&db, tables)
        .await
        .unwrap()
        .info
        .unwrap()
        .resource_return
        .unwrap();
    assert_eq!(rotated.last_finish_random_task_id, Some(random_task));
    assert_eq!(rotated.left_task_ids.len(), left_count);
    assert_eq!(rotated.resource_return_tasks.len(), 1);
    assert!(
        !rotated
            .resource_return_tasks
            .iter()
            .any(|task| task.id == random_task)
    );
    let yesterday = common::time::ServerTime::server_day(common::time::ServerTime::now_ms()) - 1;
    sqlx::query("UPDATE user_turnback_state SET return_random_refresh_day = ? WHERE user_id = ?")
        .bind(yesterday)
        .bind(player_id)
        .execute(&db)
        .await
        .unwrap();
    let refreshed = manager
        .info(&db, tables)
        .await
        .unwrap()
        .info
        .unwrap()
        .resource_return
        .unwrap();
    assert_eq!(refreshed.resource_return_tasks.len(), 2);
    assert_eq!(refreshed.left_task_ids.len(), left_count - 1);
    assert_eq!(refreshed.last_finish_random_task_id, Some(random_task));
    let fixed = refreshed
        .resource_return_tasks
        .iter()
        .find(|task| task.id == fixed_task)
        .unwrap();
    assert_eq!(fixed.progress, 0);
    assert!(!fixed.has_finished);
    assert_eq!(fixed.finish_count, Some(0));

    let single = rows[1];
    let claim = manager
        .claim_return_reward(&db, turnback.id, single.id, tables)
        .await
        .unwrap();
    assert_eq!(claim.reply.has_get_return_reward_ids, [single.id]);
    assert_eq!(
        claim.material_changes,
        reward::parse_strict(&single.bonus)
            .unwrap()
            .material_changes()
    );
    assert!(
        manager
            .claim_return_reward(&db, turnback.id, single.id, tables)
            .await
            .is_err()
    );

    let claim = manager
        .claim_all_return_rewards(&db, turnback.id, tables)
        .await
        .unwrap();
    let expected = rows
        .iter()
        .filter(|row| row.need_vitality <= vitality)
        .map(|row| row.id)
        .collect::<Vec<_>>();
    assert_eq!(claim.reply.has_get_return_reward_ids, expected);
    let resource = manager
        .info(&db, tables)
        .await
        .unwrap()
        .info
        .unwrap()
        .resource_return
        .unwrap();
    assert!(
        resource
            .rewards
            .iter()
            .filter(|reward| expected.contains(&reward.reward_id.unwrap()))
            .all(|reward| reward.state == Some(2))
    );

    currencies::add_currency(
        &db,
        player_id,
        currency_id,
        rows[3].need_vitality - vitality,
    )
    .await
    .unwrap();
    sqlx::query("UPDATE user_turnback_state SET end_time = ? WHERE user_id = ?")
        .bind(now - 1)
        .bind(player_id)
        .execute(&db)
        .await
        .unwrap();
    assert!(
        manager
            .claim_return_reward(&db, turnback.id, rows[3].id, tables)
            .await
            .is_err()
    );
    assert_eq!(
        manager
            .info(&db, tables)
            .await
            .unwrap()
            .info
            .unwrap()
            .resource_return
            .unwrap()
            .open,
        Some(false)
    );
}

#[test]
fn return_reward_range_boundaries_are_config_owned() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let mut ranges = tables.turnback_return_range.iter().collect::<Vec<_>>();
    ranges.sort_by_key(|row| row.range_id);
    for row in ranges {
        let start = row
            .loss_days
            .split_once('|')
            .unwrap()
            .0
            .parse::<i32>()
            .unwrap();
        assert_eq!(
            super::return_rewards::range_id(start, tables).unwrap(),
            row.range_id
        );
    }
}

#[tokio::test]
async fn malformed_claim_history_is_not_replaced() {
    let player_id = 74;
    let db = database(player_id).await;
    let tables = config::configs::get();
    let bonus = tables.turnback_task_bonus.iter().next().unwrap();
    sqlx::query(
        "INSERT INTO user_turnback_state
            (user_id, turnback_id, bonus_point, has_get_task_bonus)
         VALUES (?, ?, ?, '{')",
    )
    .bind(player_id)
    .bind(bonus.turnback_id)
    .bind(bonus.need_point)
    .execute(&db)
    .await
    .unwrap();

    let mut tx = db.begin().await.unwrap();
    let result =
        claim_turnback_bonus_ids(&mut tx, player_id, bonus.turnback_id, [bonus.id], tables).await;
    assert!(matches!(result, Err(AppError::Serde(_))));
    tx.rollback().await.unwrap();

    let stored: String =
        sqlx::query_scalar("SELECT has_get_task_bonus FROM user_turnback_state WHERE user_id = ?")
            .bind(player_id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(stored, "{");

    let turnback = tables
        .turnback
        .iter()
        .find(|row| !row.resource_return_condition.is_empty())
        .unwrap();
    let now = common::time::ServerTime::now_sec_i32();
    sqlx::query(
        "UPDATE user_turnback_state
         SET turnback_id = ?, start_time = ?, end_time = ?, leave_time = ?,
             has_get_return_reward_ids = '{'
         WHERE user_id = ?",
    )
    .bind(turnback.id)
    .bind(now)
    .bind(now + 86_400)
    .bind(now - 43 * 86_400)
    .bind(player_id)
    .execute(&db)
    .await
    .unwrap();
    let manager = TurnbackManager::new(player_id);
    assert!(
        manager
            .claim_all_return_rewards(&db, turnback.id, tables)
            .await
            .is_err()
    );
    let stored: String = sqlx::query_scalar(
        "SELECT has_get_return_reward_ids FROM user_turnback_state WHERE user_id = ?",
    )
    .bind(player_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(stored, "{");
}
