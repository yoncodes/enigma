use super::{add_missing_leaf_groups, apply_state, build_red_dot_children, loadable_leaf_ids};
use crate::{bp::BattlePassManager, types::red_dot_id::RedDotId};
use database::{
    db::game::{
        battle_pass,
        tasks::{self as task_db, TaskType},
    },
    models::game::red_dots::RedDotRecord,
};
use sonettobuf::{GetRedDotInfosReply, RedDotGroup, RedDotInfo};
use sqlx::sqlite::SqlitePoolOptions;

#[test]
fn parses_parent_links_from_reddot_table() {
    let children = build_red_dot_children([
        (13, ""),
        (1013, "13#1012#1052"),
        (1042, "13#1051"),
        (1902, "13"),
    ]);

    assert_eq!(children.get(&13).unwrap(), &vec![1013, 1042, 1902]);

    let mut reply = GetRedDotInfosReply {
        red_dot_infos: Vec::new(),
    };
    add_missing_leaf_groups(&mut reply, &[13, 1013, 1042, 1902, 2307], &children);
    assert_eq!(
        reply
            .red_dot_infos
            .iter()
            .map(|group| group.define_id)
            .collect::<Vec<_>>(),
        vec![1013, 1042, 1902, 2307]
    );
    assert!(
        reply
            .red_dot_infos
            .iter()
            .all(|group| group.infos[0].value == 0)
    );
}

#[test]
fn full_catalog_only_loads_online_server_leaf_nodes() {
    let children = build_red_dot_children([(1, ""), (2, "1"), (3, "")]);

    assert_eq!(
        loadable_leaf_ids([(1, 1, 1), (2, 1, 1), (3, 0, 1), (4, 1, 0)], &children),
        vec![2]
    );
}

#[test]
fn state_zero_removes_static_info() {
    let mut reply = GetRedDotInfosReply {
        red_dot_infos: vec![RedDotGroup {
            define_id: 1002,
            infos: vec![RedDotInfo {
                id: 0,
                value: 1,
                time: Some(1),
                ext: Some(String::new()),
            }],
            replace_all: Some(true),
        }],
    };

    apply_state(
        &mut reply,
        RedDotRecord {
            id: 1,
            player_id: 1,
            define_id: 1002,
            info_id: 0,
            value: 0,
            time: 0,
            ext: String::new(),
            replace_all: false,
            created_at: 0,
            updated_at: 0,
        },
    );

    assert!(reply.red_dot_infos.is_empty());
}

#[test]
fn empty_replacement_removes_bp_task_red_dot() {
    let mut reply = GetRedDotInfosReply {
        red_dot_infos: vec![RedDotGroup {
            define_id: RedDotId::BattlePassTask.id(),
            infos: vec![RedDotInfo {
                id: 0,
                value: 1,
                time: Some(0),
                ext: Some(String::new()),
            }],
            replace_all: Some(true),
        }],
    };

    super::replace_group(&mut reply, RedDotId::BattlePassTask.id(), vec![]);

    assert!(reply.red_dot_infos.is_empty());
}

#[tokio::test]
async fn task_only_bp_red_dot_initializes_the_selected_pass() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (1, 'bp-red-dot', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    battle_pass::get_or_create_state(&pool, 1, 28)
        .await
        .unwrap();

    BattlePassManager::new(1)
        .task_red_dot_infos_for(&pool, 26)
        .await
        .unwrap();

    let bp_ids =
        sqlx::query_scalar::<_, i32>("SELECT bp_id FROM user_battle_pass_state WHERE user_id = 1")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(bp_ids, vec![26]);
    for task_type in [TaskType::BattlePass, TaskType::BpOperAct] {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM user_tasks WHERE user_id = 1 AND type_id = ?",
        )
        .bind(task_type.id())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(count > 0, "missing {task_type:?} tasks");
    }
}

#[tokio::test]
async fn version_activity_red_dot_follows_claimable_task_state() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (4, 'version-task-red-dot', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    task_db::ensure_tasks_for_type(&pool, 4, TaskType::VersionActivity)
        .await
        .unwrap();
    let task_id = sqlx::query_scalar::<_, i32>(
        "SELECT task_id FROM user_tasks
         WHERE user_id = 4 AND type_id = ?
         ORDER BY task_id LIMIT 1",
    )
    .bind(TaskType::VersionActivity.id())
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE user_tasks SET progress = 1
         WHERE user_id = 4 AND type_id = ? AND task_id = ?",
    )
    .bind(TaskType::VersionActivity.id())
    .bind(task_id)
    .execute(&pool)
    .await
    .unwrap();

    let manager = super::RedDotManager::new(4);
    let partial = manager
        .infos(&pool, vec![RedDotId::CommandStationTaskNormal.id()])
        .await
        .unwrap();
    assert_eq!(partial.red_dot_infos[0].infos[0].value, 0);

    sqlx::query(
        "UPDATE user_tasks SET has_finished = 1
         WHERE user_id = 4 AND type_id = ? AND task_id = ?",
    )
    .bind(TaskType::VersionActivity.id())
    .bind(task_id)
    .execute(&pool)
    .await
    .unwrap();

    let claimable = manager
        .infos(&pool, vec![RedDotId::CommandStationTaskNormal.id()])
        .await
        .unwrap();
    assert_eq!(claimable.red_dot_infos[0].infos[0].value, 1);

    task_db::finish_task(&pool, 4, task_id)
        .await
        .unwrap()
        .unwrap();
    let claimed = manager
        .infos(&pool, vec![RedDotId::CommandStationTaskNormal.id()])
        .await
        .unwrap();
    assert_eq!(claimed.red_dot_infos[0].infos[0].value, 0);
}

#[tokio::test]
async fn achievement_red_dot_uses_remaining_new_categories() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (1, 'red-dot', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    for (id, is_new) in [(41040201, false), (41170101, true)] {
        sqlx::query(
            "INSERT INTO user_achievements
             (user_id, achievement_id, progress, has_finish, is_new, finish_time, created_at, updated_at)
             VALUES (1, ?, 1, 1, ?, 1, 1, 1)",
        )
        .bind(id)
        .bind(is_new)
        .execute(&pool)
        .await
        .unwrap();
    }

    let manager = super::RedDotManager::new(1);
    let reply = manager
        .infos(&pool, vec![RedDotId::AchievementFinish.id()])
        .await
        .unwrap();
    assert_eq!(reply.red_dot_infos[0].infos[0].id, 4);

    database::db::game::achievements::clear_new_flags(&pool, 1, vec![41170101])
        .await
        .unwrap();
    let reply = manager
        .infos(&pool, vec![RedDotId::AchievementFinish.id()])
        .await
        .unwrap();
    assert_eq!(
        (
            reply.red_dot_infos[0].infos[0].id,
            reply.red_dot_infos[0].infos[0].value
        ),
        (0, 0)
    );
}

#[tokio::test]
async fn act233_red_dots_follow_claimable_tasks_and_bonus_state() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (6, 'act233-dots', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let pass = config::configs::get().activity233_bp.iter().next().unwrap();

    let (task_infos, bonus) = super::act233_red_dot_state(&pool, 6, Some(pass.activity_id))
        .await
        .unwrap();
    assert_eq!((task_infos[0].id, bonus), (0, 0));

    sqlx::query(
        "UPDATE user_act233_bp_state SET score = ?
         WHERE user_id = 6 AND activity_id = ? AND bp_id = ?",
    )
    .bind(pass.exp_level_up * 2)
    .bind(pass.activity_id)
    .bind(pass.bp_id)
    .execute(&pool)
    .await
    .unwrap();
    let (_, bonus) = super::act233_red_dot_state(&pool, 6, Some(pass.activity_id))
        .await
        .unwrap();
    assert_eq!(bonus, 1);

    sqlx::query(
        "UPDATE user_act233_bp_state SET has_get_free_bonus = '[1,2]'
         WHERE user_id = 6 AND activity_id = ? AND bp_id = ?",
    )
    .bind(pass.activity_id)
    .bind(pass.bp_id)
    .execute(&pool)
    .await
    .unwrap();
    let (_, bonus) = super::act233_red_dot_state(&pool, 6, Some(pass.activity_id))
        .await
        .unwrap();
    assert_eq!(bonus, 0);

    let task = config::configs::get()
        .activity233_task
        .iter()
        .find(|task| {
            task.activity_id == pass.activity_id && task.bp_id == pass.bp_id && task.is_online != 0
        })
        .unwrap();
    sqlx::query(
        "UPDATE user_tasks SET progress = 0, finish_count = 0
         WHERE user_id = 6 AND type_id = ? AND activity_id = ?",
    )
    .bind(database::db::game::tasks::TaskType::ActBp.id())
    .bind(pass.activity_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE user_tasks SET progress = ?
         WHERE user_id = 6 AND type_id = ? AND task_id = ?",
    )
    .bind(task.max_progress)
    .bind(database::db::game::tasks::TaskType::ActBp.id())
    .bind(task.id)
    .execute(&pool)
    .await
    .unwrap();

    let (task_infos, _) = super::act233_red_dot_state(&pool, 6, Some(pass.activity_id))
        .await
        .unwrap();
    let loop_type = database::db::game::tasks::TaskLoopType::from_id(task.loop_type)
        .unwrap_or(database::db::game::tasks::TaskLoopType::Permanent);
    let expected_id = match loop_type {
        database::db::game::tasks::TaskLoopType::Appoint => {
            database::db::game::tasks::TaskLoopType::Permanent.id() as i64
        }
        other => other.id() as i64,
    };
    assert_eq!(task_infos, vec![super::red_dot_info(expected_id, 1)]);

    let successor = config::configs::get()
        .activity233_task
        .iter()
        .find(|task| {
            task.activity_id == pass.activity_id
                && task.bp_id == pass.bp_id
                && !task.prepose.is_empty()
        })
        .unwrap();
    let predecessor_id = successor.prepose.parse::<i32>().unwrap();
    let successor_loop = database::db::game::tasks::TaskLoopType::from_id(successor.loop_type)
        .unwrap_or(database::db::game::tasks::TaskLoopType::Permanent);
    let successor_info_id = match successor_loop {
        database::db::game::tasks::TaskLoopType::Appoint => {
            database::db::game::tasks::TaskLoopType::Permanent.id() as i64
        }
        other => other.id() as i64,
    };
    sqlx::query(
        "UPDATE user_tasks SET progress = 0, finish_count = 0
         WHERE user_id = 6 AND type_id = ? AND activity_id = ?",
    )
    .bind(database::db::game::tasks::TaskType::ActBp.id())
    .bind(pass.activity_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE user_tasks SET progress = ?
         WHERE user_id = 6 AND type_id = ? AND task_id = ?",
    )
    .bind(successor.max_progress)
    .bind(database::db::game::tasks::TaskType::ActBp.id())
    .bind(successor.id)
    .execute(&pool)
    .await
    .unwrap();

    let (task_infos, _) = super::act233_red_dot_state(&pool, 6, Some(pass.activity_id))
        .await
        .unwrap();
    assert_eq!(task_infos, vec![super::red_dot_info(0, 0)]);

    sqlx::query(
        "UPDATE user_tasks SET finish_count = 1
         WHERE user_id = 6 AND type_id = ? AND task_id = ?",
    )
    .bind(database::db::game::tasks::TaskType::ActBp.id())
    .bind(predecessor_id)
    .execute(&pool)
    .await
    .unwrap();
    let (task_infos, _) = super::act233_red_dot_state(&pool, 6, Some(pass.activity_id))
        .await
        .unwrap();
    assert_eq!(task_infos, vec![super::red_dot_info(successor_info_id, 1)]);
}

#[tokio::test]
async fn room_faith_red_dot_is_derived_from_accumulated_minutes() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (2, 'faith-dot', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_room_heroes (user_id, hero_id, current_minute)
         VALUES (2, 3023, 1199)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let define_id = RedDotId::RoomCharacterFaithGetFull.id();
    let manager = super::RedDotManager::new(2);
    let before = manager.infos(&pool, vec![define_id]).await.unwrap();
    assert_eq!(before.red_dot_infos[0].infos[0].id, 0);
    assert_eq!(before.red_dot_infos[0].infos[0].value, 0);

    sqlx::query(
        "UPDATE user_room_heroes SET current_minute = 1200
         WHERE user_id = 2 AND hero_id = 3023",
    )
    .execute(&pool)
    .await
    .unwrap();
    let full = manager.infos(&pool, vec![define_id]).await.unwrap();
    assert_eq!(full.red_dot_infos[0].infos[0].id, 3023);
    assert_eq!(full.red_dot_infos[0].infos[0].value, 1);
    assert_eq!(full.red_dot_infos[0].replace_all, Some(true));

    sqlx::query(
        "INSERT INTO user_room_production_lines
         (user_id, line_id, formula_id, finish_count, level)
         VALUES (2, 1, 2002001, 999, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let production = manager
        .infos(&pool, vec![RedDotId::RoomProductionFull.id()])
        .await
        .unwrap();
    assert_eq!(production.red_dot_infos[0].infos[0].id, 1);
    assert_eq!(production.red_dot_infos[0].infos[0].value, 1);
}

#[tokio::test]
async fn trade_order_red_dot_is_derived_from_saved_orders_and_inventory() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (3, 'trade-dot', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::starter_data::load_all_starter_data(&pool, 3)
        .await
        .unwrap();

    let define_id = RedDotId::TradeOrderFulfillable.id();
    let manager = super::RedDotManager::new(3);
    let before = manager.infos(&pool, vec![define_id]).await.unwrap();
    assert_eq!(before.red_dot_infos[0].infos[0].value, 0);

    let goods = sqlx::query_as::<_, (i32, i32)>(
        "SELECT production_id, quantity FROM user_room_purchase_order_goods
         WHERE user_id = 3 AND order_id = 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    for (production_id, quantity) in goods {
        let item_id = config::configs::get()
            .manufacture_item
            .get(production_id)
            .unwrap()
            .item_id;
        database::db::game::items::add_item_quantity(&pool, 3, item_id as u32, quantity)
            .await
            .unwrap();
    }

    let ready = manager.infos(&pool, vec![define_id]).await.unwrap();
    assert_eq!(ready.red_dot_infos[0].infos[0].value, 1);
    assert_eq!(manager.trade_order_value(&pool).await.unwrap(), 1);
}
