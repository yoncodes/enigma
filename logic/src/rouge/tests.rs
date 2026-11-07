use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

use super::RougeManager;
use crate::error::AppError;

async fn test_pool() -> SqlitePool {
    crate::init_test_server_config();
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
         VALUES (1, 'rouge2-alchemy', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    RougeManager::new(1).outside_info(&pool).await.unwrap();
    pool
}

#[tokio::test]
async fn difficulty_initializes_and_persists_the_captured_run_state() {
    let pool = test_pool().await;
    let manager = RougeManager::new(1);
    seed_materials(&pool).await;
    manager.alchemy(&pool, 103, vec![1101]).await.unwrap();
    assert_eq!(
        manager.info(&pool).await.unwrap().rouge2_info,
        Some(sonettobuf::Rouge2Info::default())
    );

    let result = manager.select_difficulty(&pool, 1).await.unwrap();
    assert_eq!(
        result
            .updates
            .iter()
            .map(|attribute| (attribute.attr_id.unwrap(), attribute.final_val.unwrap()))
            .collect::<Vec<_>>(),
        vec![
            (320, 1),
            (321, 1000),
            (326, 10000),
            (202, 99),
            (204, 99),
            (205, 3),
            (310, 500),
            (311, 100),
            (312, 100),
            (313, 50),
            (314, 50),
            (315, 100),
            (316, 500),
            (317, 3000),
            (318, 50),
            (319, 100),
        ]
    );
    let info = result.reply.rouge2_info.unwrap();
    assert_eq!(info.state, Some(1));
    assert_eq!(info.difficulty, Some(1));
    assert_eq!(info.game_num, Some(1));
    assert_eq!(
        info.bag_info
            .as_ref()
            .unwrap()
            .bags
            .iter()
            .map(|bag| bag.bag_type.unwrap())
            .collect::<Vec<_>>(),
        vec![1, 3, 4]
    );
    assert_eq!(
        info.leader_info.as_ref().unwrap().equip_active_skill,
        vec![0, 0, 0]
    );
    assert_eq!(
        info.attr_info.as_ref().unwrap().attr.len(),
        common::config().rouge2.initial_attributes.len()
    );
    assert_eq!(
        info.attr_info
            .as_ref()
            .unwrap()
            .attr
            .iter()
            .map(|attribute| (attribute.attr_id.unwrap(), attribute.final_val.unwrap()))
            .collect::<Vec<_>>(),
        vec![
            (513, 0),
            (514, 0),
            (131, 0),
            (132, 0),
            (133, 0),
            (141, 0),
            (142, 0),
            (143, 0),
            (401, 0),
            (402, 0),
            (403, 0),
            (404, 0),
            (405, 0),
            (406, 0),
            (407, 0),
            (408, 0),
            (409, 0),
            (410, 0),
            (415, 0),
            (421, 0),
            (301, 0),
            (310, 500),
            (311, 100),
            (312, 100),
            (313, 50),
            (314, 50),
            (315, 100),
            (316, 500),
            (317, 3000),
            (318, 50),
            (319, 100),
            (320, 1),
            (321, 1000),
            (322, 0),
            (323, 0),
            (324, 0),
            (325, 0),
            (326, 10000),
            (327, 0),
            (329, 0),
            (202, 99),
            (330, 0),
            (331, 0),
            (204, 99),
            (332, 0),
            (205, 3),
            (333, 0),
            (334, 0),
            (601, 0),
            (611, 0),
            (612, 0),
            (101, 0),
            (102, 0),
            (103, 0),
            (104, 0),
            (105, 0),
            (106, 0),
            (501, 0),
            (121, 0),
            (122, 0),
            (123, 0),
        ]
    );
    assert_eq!(info.alchemy_info.as_ref().unwrap().sub_effect, vec![190901]);
    assert_eq!(manager.info(&pool).await.unwrap().rouge2_info, Some(info));

    sqlx::query("UPDATE user_rouge2_state SET state = 0 WHERE user_id = 1")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        manager
            .select_difficulty(&pool, 1)
            .await
            .unwrap()
            .reply
            .rouge2_info
            .unwrap()
            .game_num,
        Some(2)
    );
}

#[tokio::test]
async fn difficulty_rejects_locked_or_active_run_without_mutation() {
    let pool = test_pool().await;
    let manager = RougeManager::new(1);
    assert!(matches!(
        manager.select_difficulty(&pool, 2).await,
        Err(AppError::InvalidRequest)
    ));
    assert_eq!(
        sqlx::query_as::<_, (i32, i32, i32)>(
            "SELECT state, difficulty, game_num FROM user_rouge2_state WHERE user_id = 1"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        (0, 0, 0)
    );

    sqlx::query("UPDATE user_rouge2_state SET max_difficulty = 1 WHERE user_id = 1")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        manager.select_difficulty(&pool, 2).await,
        Err(AppError::InvalidRequest)
    ));
    assert_eq!(
        sqlx::query_as::<_, (i32, i32, i32)>(
            "SELECT state, difficulty, game_num FROM user_rouge2_state WHERE user_id = 1"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        (0, 0, 0)
    );

    manager.select_difficulty(&pool, 1).await.unwrap();
    let before = manager.info(&pool).await.unwrap();
    assert!(matches!(
        manager.select_difficulty(&pool, 1).await,
        Err(AppError::InvalidRequest)
    ));
    assert_eq!(manager.info(&pool).await.unwrap(), before);
}

#[tokio::test]
async fn genius_activation_follows_the_common_tree_and_spends_points() {
    let pool = test_pool().await;
    let manager = RougeManager::new(1);
    sqlx::query("UPDATE user_rouge2_state SET state = 1, genius_point = 2 WHERE user_id = 1")
        .execute(&pool)
        .await
        .unwrap();

    let difficulty = genius_snapshot(&pool).await;
    assert!(matches!(
        manager.activate_genius(&pool, 10009).await,
        Err(AppError::InvalidRequest)
    ));
    assert_eq!(genius_snapshot(&pool).await, difficulty);
    sqlx::query("UPDATE user_rouge2_state SET state = 2 WHERE user_id = 1")
        .execute(&pool)
        .await
        .unwrap();
    let active = genius_snapshot(&pool).await;
    assert!(matches!(
        manager.activate_genius(&pool, 10009).await,
        Err(AppError::InvalidRequest)
    ));
    assert_eq!(genius_snapshot(&pool).await, active);
    sqlx::query("UPDATE user_rouge2_state SET state = 0 WHERE user_id = 1")
        .execute(&pool)
        .await
        .unwrap();

    assert!(matches!(
        manager.activate_genius(&pool, 10010).await,
        Err(AppError::InvalidRequest)
    ));
    let first = manager.activate_genius(&pool, 10009).await.unwrap();
    assert_eq!(first.genius_point, 1);
    assert_eq!(first.reply.genius_id, Some(10009));
    assert_eq!(first.reply.genius_ids, vec![10009]);
    assert!(matches!(
        manager.activate_genius(&pool, 10009).await,
        Err(AppError::InvalidRequest)
    ));

    let second = manager.activate_genius(&pool, 10010).await.unwrap();
    assert_eq!(second.genius_point, 0);
    assert_eq!(second.reply.genius_ids, vec![10009, 10010]);
    let exhausted = genius_snapshot(&pool).await;
    assert!(matches!(
        manager.activate_genius(&pool, 10013).await,
        Err(AppError::InvalidRequest)
    ));
    assert_eq!(genius_snapshot(&pool).await, exhausted);
    assert!(matches!(
        manager.activate_genius(&pool, 2000).await,
        Err(AppError::InvalidRequest)
    ));
    assert_eq!(
        sqlx::query_as::<_, (i32, String)>(
            "SELECT genius_point, genius_ids FROM user_rouge2_state WHERE user_id = 1"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        (0, "[10009,10010]".into())
    );
}

async fn genius_snapshot(pool: &SqlitePool) -> (i32, i32, String) {
    sqlx::query_as(
        "SELECT state, genius_point, genius_ids FROM user_rouge2_state WHERE user_id = 1",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_materials(pool: &SqlitePool) {
    for (material_id, num) in [(1008, 1), (1000, 4), (1001, 1), (1101, 1), (1103, 0)] {
        sqlx::query(
            "INSERT INTO user_rouge2_materials
             (user_id, material_id, num, is_seen, updated_at)
             VALUES (1, ?, ?, 1, 0)",
        )
        .bind(material_id)
        .bind(num)
        .execute(pool)
        .await
        .unwrap();
    }
}

async fn snapshot(pool: &SqlitePool) -> ((i32, i32, i32, String), Vec<(i32, i32, bool)>) {
    let state = sqlx::query_as(
        "SELECT state, alchemy_formula, genius_point, alchemy_sub_effects
         FROM user_rouge2_state WHERE user_id = 1",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let materials = sqlx::query_as(
        "SELECT material_id, num, is_seen
         FROM user_rouge2_materials WHERE user_id = 1 ORDER BY rowid",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    (state, materials)
}

#[tokio::test]
async fn alchemy_consumes_owned_materials_and_persists_the_result() {
    let pool = test_pool().await;
    seed_materials(&pool).await;

    let result = RougeManager::new(1)
        .alchemy(&pool, 103, vec![1101])
        .await
        .unwrap();

    assert_eq!(result.genius_point, 1);
    let info = result.reply.alchemy_info.unwrap();
    let current = info.cur_alchemy_info.unwrap();
    assert_eq!(current.formula, Some(103));
    assert_eq!(current.main_effect, Some(190047));
    assert_eq!(current.sub_effect, vec![190901]);
    assert_eq!(
        info.alchemy_material_info
            .iter()
            .map(|item| (item.id.unwrap(), item.num.unwrap()))
            .collect::<Vec<_>>(),
        vec![(1008, 1), (1000, 3), (1001, 0), (1101, 0), (1103, 0)]
    );

    let outside = RougeManager::new(1).outside_info(&pool).await.unwrap();
    assert_eq!(
        outside
            .outside_info
            .unwrap()
            .alchemy_info
            .unwrap()
            .cur_alchemy_info
            .unwrap(),
        current
    );
}

#[tokio::test]
async fn alchemy_rejections_do_not_consume_materials() {
    let pool = test_pool().await;
    seed_materials(&pool).await;
    let initial = snapshot(&pool).await;

    assert!(matches!(
        RougeManager::new(1)
            .alchemy(&pool, 103, vec![1101, 1101])
            .await,
        Err(AppError::InvalidRequest)
    ));
    assert!(matches!(
        RougeManager::new(1).alchemy(&pool, 103, Vec::new()).await,
        Err(AppError::InvalidRequest)
    ));
    assert!(matches!(
        RougeManager::new(1).alchemy(&pool, 104, vec![1101]).await,
        Err(AppError::InvalidRequest)
    ));
    assert_eq!(snapshot(&pool).await, initial);

    sqlx::query("UPDATE user_rouge2_state SET state = 1 WHERE user_id = 1")
        .execute(&pool)
        .await
        .unwrap();
    let active = snapshot(&pool).await;
    assert!(matches!(
        RougeManager::new(1).alchemy(&pool, 103, vec![1101]).await,
        Err(AppError::InvalidRequest)
    ));
    assert_eq!(snapshot(&pool).await, active);

    sqlx::query("UPDATE user_rouge2_state SET state = 0 WHERE user_id = 1")
        .execute(&pool)
        .await
        .unwrap();
    let conditional = snapshot(&pool).await;
    assert!(matches!(
        RougeManager::new(1).alchemy(&pool, 101, vec![1101]).await,
        Err(AppError::InvalidRequest)
    ));
    assert_eq!(snapshot(&pool).await, conditional);

    sqlx::query("UPDATE user_rouge2_materials SET num = 0 WHERE material_id = 1001")
        .execute(&pool)
        .await
        .unwrap();
    let insufficient = snapshot(&pool).await;
    assert!(matches!(
        RougeManager::new(1).alchemy(&pool, 103, vec![1101]).await,
        Err(AppError::InsufficientItems)
    ));
    assert_eq!(snapshot(&pool).await, insufficient);
}
