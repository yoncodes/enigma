use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

use super::RougeManager;
use crate::error::AppError;

async fn test_pool() -> SqlitePool {
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
