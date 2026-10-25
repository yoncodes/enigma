use super::*;
use sonettobuf::{ArcadeExtendInfo, ArcadeInSideProp};

async fn test_pool(player_id: i64) -> SqlitePool {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
        .bind(player_id)
        .bind(format!("arcade-{player_id}"))
        .execute(&pool)
        .await
        .unwrap();
    pool
}

fn inside_run(difficulty: i32, diamond: i32, cassette: i32) -> ArcadeInSideInfo {
    ArcadeInSideInfo {
        player: Some(ArcadePlayer {
            id: Some(101),
            ..Default::default()
        }),
        attr_container: Some(ArcadeAttrContainer {
            attr_values: vec![
                ArcadeAttrValue {
                    id: Some(DIAMOND_ATTR_ID),
                    base: Some(diamond),
                    ..Default::default()
                },
                ArcadeAttrValue {
                    id: Some(CASSETTE_ATTR_ID),
                    base: Some(cassette),
                    ..Default::default()
                },
            ],
        }),
        prop: Some(ArcadeInSideProp {
            area_id: Some(0),
            room_id: Some(10001),
            progress: Some(0),
            difficulty: Some(difficulty),
            ..Default::default()
        }),
        extend_info: Some(ArcadeExtendInfo {
            added_book: Some(ArcadeBookInfo {
                books: vec![
                    ArcadeBook {
                        r#type: Some(2),
                        ele_id: vec![10009, 10004],
                        ..Default::default()
                    },
                    ArcadeBook {
                        r#type: Some(3),
                        ele_id: vec![106],
                        ..Default::default()
                    },
                    ArcadeBook {
                        r#type: Some(4),
                        ele_id: vec![
                            200015, 200007, 210002, 200001, 210004, 200004, 200002, 200003, 200005,
                            200008, 210001, 210003, 200006,
                        ],
                        ..Default::default()
                    },
                ],
            }),
            unlock_difficulty_ids: vec![difficulty + 1],
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[tokio::test]
async fn fresh_snapshot_matches_captured_presence_and_config_defaults() {
    let pool = test_pool(5651).await;
    let info = ArcadeOutsideManager::new(5651)
        .info(&pool, config::configs::get())
        .await
        .unwrap()
        .info
        .unwrap();

    assert_eq!(info.activity_id, None);
    assert_eq!(info.board_info.unwrap().cells, vec![]);
    assert_eq!(info.talent_info.unwrap().talents, vec![]);
    assert_eq!(info.unit_info.unwrap().units, vec![]);
    assert_eq!(info.attr_container.unwrap().attr_values, vec![]);
    let books = info.book_info.unwrap().books;
    assert_eq!(
        books
            .iter()
            .map(|book| book.r#type.unwrap())
            .collect::<Vec<_>>(),
        vec![4, 1, 2, 3]
    );
    assert_eq!(books[1].ele_id, vec![101]);
    assert_eq!(books[1].new_ele_id, vec![101]);
    assert_eq!(books[2].ele_id, vec![16005]);
    assert_eq!(books[2].new_ele_id, vec![16005]);
    let player = info.player.unwrap();
    assert_eq!(player.id, Some(101));
    assert_eq!(
        player.pos,
        Some(ArcadePos {
            x: Some(6),
            y: Some(4)
        })
    );
    assert_eq!(player.dir, Some(0));
    assert_eq!(info.prop.unwrap().unlock_difficulty_ids, vec![1]);
}

#[tokio::test]
async fn movement_is_bounded_and_persistent_and_locked_switch_is_rejected() {
    let pool = test_pool(5652).await;
    let manager = ArcadeOutsideManager::new(5652);
    manager.info(&pool, config::configs::get()).await.unwrap();
    assert!(matches!(
        manager
            .move_player(&pool, config::configs::get(), 0, 4)
            .await,
        Err(AppError::InvalidRequest)
    ));
    assert_eq!(
        manager
            .move_player(&pool, config::configs::get(), 12, 12)
            .await
            .unwrap(),
        ArcadePlayerMoveReply {
            x: Some(12),
            y: Some(12)
        }
    );
    let player = manager
        .info(&pool, config::configs::get())
        .await
        .unwrap()
        .info
        .unwrap()
        .player
        .unwrap();
    assert_eq!(
        player.pos,
        Some(ArcadePos {
            x: Some(12),
            y: Some(12)
        })
    );
    assert!(matches!(
        manager
            .switch_character(&pool, config::configs::get(), 102)
            .await,
        Err(AppError::InvalidRequest)
    ));
}

#[tokio::test]
async fn talent_upgrade_uses_current_level_and_atomically_spends_diamonds() {
    let pool = test_pool(5653).await;
    let manager = ArcadeOutsideManager::new(5653);
    manager.info(&pool, config::configs::get()).await.unwrap();
    let activity_id = config::configs::get()
        .latest_open_activity_id(ACTIVITY_TYPE_ID)
        .unwrap();
    sqlx::query("INSERT INTO user_arcade_attrs (user_id, activity_id, attr_id, base, rate, extra) VALUES (5653, ?, 202, 240, 0, 0)")
        .bind(activity_id).execute(&pool).await.unwrap();

    let upgrade = manager
        .upgrade_talent(&pool, config::configs::get(), 100, 0)
        .await
        .unwrap();
    assert_eq!(upgrade.reply.level, Some(1));
    assert_eq!(
        upgrade.changed_attr,
        ArcadeAttrValue {
            id: Some(202),
            base: Some(190),
            rate: Some(0),
            extra: None
        }
    );
    assert!(matches!(
        manager
            .upgrade_talent(&pool, config::configs::get(), 100, 0)
            .await,
        Err(AppError::InvalidRequest)
    ));
    assert_eq!(sqlx::query_scalar::<_, i32>("SELECT base FROM user_arcade_attrs WHERE user_id = 5653 AND activity_id = ? AND attr_id = 202").bind(activity_id).fetch_one(&pool).await.unwrap(), 190);
}

#[tokio::test]
async fn reward_zero_claims_all_eligible_tiers_once() {
    let pool = test_pool(5654).await;
    let manager = ArcadeOutsideManager::new(5654);
    manager.info(&pool, config::configs::get()).await.unwrap();
    let activity_id = config::configs::get()
        .latest_open_activity_id(ACTIVITY_TYPE_ID)
        .unwrap();
    sqlx::query(
        "UPDATE user_arcade_outside SET score = 1000 WHERE user_id = 5654 AND activity_id = ?",
    )
    .bind(activity_id)
    .execute(&pool)
    .await
    .unwrap();

    let first = manager
        .gain_rewards(&pool, config::configs::get(), 0)
        .await
        .unwrap();
    assert!(!first.material_changes.is_empty());
    assert_eq!(first.red_dot_id, 3306);
    assert_eq!(
        arcade::get_claims(&pool, 5654, activity_id).await.unwrap(),
        vec![37001]
    );
    let second = manager
        .gain_rewards(&pool, config::configs::get(), 0)
        .await
        .unwrap();
    assert!(second.material_changes.is_empty());
    assert!(matches!(
        manager
            .gain_rewards(&pool, config::configs::get(), 37001)
            .await,
        Err(AppError::InvalidRequest)
    ));
}

#[tokio::test]
async fn inside_save_round_trips_and_settlement_is_atomic() {
    let pool = test_pool(5671).await;
    let manager = ArcadeOutsideManager::new(5671);
    let tables = config::configs::get();
    assert_eq!(
        manager
            .inside_info(&pool, tables)
            .await
            .unwrap()
            .has_save_game,
        Some(false)
    );

    let checkpoint = inside_run(0, 90, 1500);
    let run = inside_run(0, 240, 2500);
    manager
        .save_inside(&pool, tables, checkpoint.clone())
        .await
        .unwrap();
    let saved = manager.inside_info(&pool, tables).await.unwrap();
    assert_eq!(saved.has_save_game, Some(true));
    assert_eq!(saved.info, Some(checkpoint));
    let activity_id = tables.latest_open_activity_id(ACTIVITY_TYPE_ID).unwrap();
    sqlx::query(
        "INSERT INTO user_arcade_attrs
         (user_id, activity_id, attr_id, base, rate, extra)
         VALUES (5671, ?, 202, 100, 0, 0)",
    )
    .bind(activity_id)
    .execute(&pool)
    .await
    .unwrap();

    let settlement = manager
        .settle_inside(&pool, tables, 2, run.clone())
        .await
        .unwrap();
    assert_eq!(settlement.reply.book_add_score, Some(400));
    assert_eq!(settlement.reply.unlock_role_ids, Vec::<i32>::new());
    assert_eq!(settlement.reply.hotfix, None);
    assert_eq!(settlement.changed_attr.base, Some(340));
    assert!(settlement.tasks.iter().any(|task| {
        task.type_id == task_db::TaskType::VersionActivity.id() && task.task_id == 912
    }));
    assert!(
        settlement.tasks.iter().any(|task| {
            task.type_id == task_db::TaskType::ActBp.id() && task.task_id == 790012
        })
    );
    assert_eq!(settlement.red_dot_id, 3306);

    let state = arcade::get_state(&pool, 5671, activity_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.score, 2900);
    assert_eq!(
        serde_json::from_str::<Vec<String>>(&state.hotfix).unwrap(),
        vec!["0#1"]
    );
    assert!(
        arcade::get_unlock_difficulties(&pool, 5671, activity_id)
            .await
            .unwrap()
            .contains(&1)
    );
    assert!(
        arcade::get_inside_save(&pool, 5671, activity_id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        manager.settle_inside(&pool, tables, 2, run).await,
        Err(AppError::InvalidRequest)
    ));
    assert_eq!(
        arcade::get_state(&pool, 5671, activity_id)
            .await
            .unwrap()
            .unwrap()
            .score,
        2900
    );
}

#[tokio::test]
async fn settlement_rejects_a_stale_same_difficulty_checkpoint() {
    let pool = test_pool(5672).await;
    let manager = ArcadeOutsideManager::new(5672);
    let tables = config::configs::get();
    manager.info(&pool, tables).await.unwrap();
    let stale = inside_run(0, 90, 1500);
    let mut current = inside_run(0, 240, 2500);
    current.prop.as_mut().unwrap().progress = Some(1);
    manager
        .save_inside(&pool, tables, stale.clone())
        .await
        .unwrap();
    manager
        .save_inside(&pool, tables, current.clone())
        .await
        .unwrap();

    assert!(matches!(
        manager.save_inside(&pool, tables, stale.clone()).await,
        Err(AppError::InvalidRequest)
    ));

    assert!(matches!(
        manager.settle_inside(&pool, tables, 2, stale).await,
        Err(AppError::InvalidRequest)
    ));
    assert_eq!(
        manager.inside_info(&pool, tables).await.unwrap().info,
        Some(current)
    );
    let activity_id = tables.latest_open_activity_id(ACTIVITY_TYPE_ID).unwrap();
    assert_eq!(
        arcade::get_state(&pool, 5672, activity_id)
            .await
            .unwrap()
            .unwrap()
            .score,
        0
    );
}

#[tokio::test]
async fn delayed_save_with_equal_totals_cannot_replace_a_later_room() {
    let pool = test_pool(5676).await;
    let manager = ArcadeOutsideManager::new(5676);
    let tables = config::configs::get();
    let previous = inside_run(0, 90, 1500);
    let mut current = previous.clone();
    let current_prop = current.prop.as_mut().unwrap();
    current_prop.room_id = Some(10002);
    current_prop.progress = Some(1);
    current.player.as_mut().unwrap().pos = Some(ArcadePos {
        x: Some(3),
        y: Some(4),
    });

    manager
        .save_inside(&pool, tables, previous.clone())
        .await
        .unwrap();
    manager
        .save_inside(&pool, tables, current.clone())
        .await
        .unwrap();
    manager
        .save_inside(&pool, tables, current.clone())
        .await
        .unwrap();
    assert!(matches!(
        manager.save_inside(&pool, tables, previous).await,
        Err(AppError::InvalidRequest)
    ));
    assert_eq!(
        manager.inside_info(&pool, tables).await.unwrap().info,
        Some(current.clone())
    );

    let mut next_area = current;
    let next_prop = next_area.prop.as_mut().unwrap();
    next_prop.area_id = Some(1);
    next_prop.room_id = Some(20001);
    next_prop.progress = Some(0);
    manager
        .save_inside(&pool, tables, next_area.clone())
        .await
        .unwrap();
    assert_eq!(
        manager.inside_info(&pool, tables).await.unwrap().info,
        Some(next_area)
    );
}

#[tokio::test]
async fn task_failure_rolls_back_settlement_and_keeps_the_save() {
    let pool = test_pool(5673).await;
    let manager = ArcadeOutsideManager::new(5673);
    let tables = config::configs::get();
    let checkpoint = inside_run(0, 90, 1500);
    let run = inside_run(0, 240, 2500);
    manager
        .save_inside(&pool, tables, checkpoint)
        .await
        .unwrap();
    sqlx::query(
        "CREATE TRIGGER fail_arcade_task_progress
         BEFORE UPDATE OF progress ON user_tasks
         WHEN NEW.user_id = 5673 AND NEW.task_id = 912
         BEGIN SELECT RAISE(FAIL, 'forced task failure'); END",
    )
    .execute(&pool)
    .await
    .unwrap();

    assert!(manager.settle_inside(&pool, tables, 2, run).await.is_err());
    let activity_id = tables.latest_open_activity_id(ACTIVITY_TYPE_ID).unwrap();
    assert!(
        arcade::get_inside_save(&pool, 5673, activity_id)
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        arcade::get_state(&pool, 5673, activity_id)
            .await
            .unwrap()
            .unwrap()
            .score,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM user_arcade_attrs
             WHERE user_id = 5673 AND activity_id = ? AND attr_id = 202",
        )
        .bind(activity_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn difficulty_factors_keep_float_precision_until_score_settlement() {
    let pool = test_pool(5674).await;
    let manager = ArcadeOutsideManager::new(5674);
    let tables = config::configs::get();

    manager
        .save_inside(&pool, tables, inside_run(1, 300, 3500))
        .await
        .unwrap();
    manager
        .settle_inside(&pool, tables, 2, inside_run(1, 450, 4500))
        .await
        .unwrap();
    let activity_id = tables.latest_open_activity_id(ACTIVITY_TYPE_ID).unwrap();
    assert_eq!(
        arcade::get_state(&pool, 5674, activity_id)
            .await
            .unwrap()
            .unwrap()
            .score,
        4900
    );

    manager
        .save_inside(&pool, tables, inside_run(2, 450, 4500))
        .await
        .unwrap();
    manager
        .settle_inside(&pool, tables, 2, inside_run(2, 600, 4750))
        .await
        .unwrap();
    assert_eq!(
        arcade::get_state(&pool, 5674, activity_id)
            .await
            .unwrap()
            .unwrap()
            .score,
        11_075
    );
}

#[tokio::test]
async fn fail_and_abandon_do_not_increment_completion_or_tasks() {
    let pool = test_pool(5675).await;
    let manager = ArcadeOutsideManager::new(5675);
    let tables = config::configs::get();
    let mut fail = inside_run(0, 90, 1500);
    fail.extend_info
        .as_mut()
        .unwrap()
        .unlock_difficulty_ids
        .clear();
    manager
        .save_inside(&pool, tables, fail.clone())
        .await
        .unwrap();
    assert!(
        manager
            .settle_inside(&pool, tables, 3, fail)
            .await
            .unwrap()
            .tasks
            .is_empty()
    );

    let mut abandon = inside_run(0, 120, 2000);
    abandon
        .extend_info
        .as_mut()
        .unwrap()
        .unlock_difficulty_ids
        .clear();
    manager
        .save_inside(&pool, tables, abandon.clone())
        .await
        .unwrap();
    assert!(
        manager
            .settle_inside(&pool, tables, 1, abandon)
            .await
            .unwrap()
            .tasks
            .is_empty()
    );

    let activity_id = tables.latest_open_activity_id(ACTIVITY_TYPE_ID).unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM user_arcade_completions
             WHERE user_id = 5675 AND activity_id = ?",
        )
        .bind(activity_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        serde_json::from_str::<Vec<String>>(
            &arcade::get_state(&pool, 5675, activity_id)
                .await
                .unwrap()
                .unwrap()
                .hotfix,
        )
        .unwrap(),
        vec![""]
    );
}
