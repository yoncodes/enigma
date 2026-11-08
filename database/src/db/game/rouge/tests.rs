use sqlx::sqlite::SqlitePoolOptions;

use super::{
    AlchemyUpdate, DifficultyUpdate, apply_alchemy, apply_difficulty, get_or_create_rouge2_outside,
};

async fn test_pool() -> sqlx::SqlitePool {
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
         VALUES (1, 'rouge2-db', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    get_or_create_rouge2_outside(&pool, 1, config::configs::get())
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn partial_material_decrement_rolls_back() {
    let pool = test_pool().await;
    for (material_id, num) in [(1000, 1), (1001, 1)] {
        sqlx::query(
            "INSERT INTO user_rouge2_materials
             (user_id, material_id, num, is_seen, updated_at)
             VALUES (1, ?, ?, 1, 0)",
        )
        .bind(material_id)
        .bind(num)
        .execute(&pool)
        .await
        .unwrap();
    }

    let applied = apply_alchemy(
        &pool,
        1,
        AlchemyUpdate {
            expected_state: 0,
            formula: 103,
            main_effect: 190047,
            sub_effects: "[190901]",
            genius_point_gain: 1,
            costs: &[(1000, 1), (1001, 2)],
        },
    )
    .await
    .unwrap();

    assert!(applied.is_none());
    assert_eq!(
        sqlx::query_as::<_, (i32, i32)>(
            "SELECT material_id, num FROM user_rouge2_materials ORDER BY material_id"
        )
        .fetch_all(&pool)
        .await
        .unwrap(),
        vec![(1000, 1), (1001, 1)]
    );
    assert_eq!(
        sqlx::query_as::<_, (i32, i32)>(
            "SELECT alchemy_formula, genius_point FROM user_rouge2_state WHERE user_id = 1"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        (0, 0)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM user_achievements WHERE user_id = 1")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn migration_preserves_old_rows_and_backfills_visibility() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql(
        "CREATE TABLE user_rouge2_state (
            user_id INTEGER PRIMARY KEY, state INTEGER NOT NULL DEFAULT 0,
            difficulty INTEGER NOT NULL DEFAULT 0, coin INTEGER NOT NULL DEFAULT 0,
            end_id INTEGER NOT NULL DEFAULT 0, game_num INTEGER NOT NULL DEFAULT 0,
            genius_point INTEGER NOT NULL DEFAULT 0, genius_ids TEXT NOT NULL DEFAULT '[]',
            reward_point INTEGER NOT NULL DEFAULT 0, max_difficulty INTEGER NOT NULL DEFAULT 0,
            pass_layer_ids TEXT NOT NULL DEFAULT '[]', pass_event_ids TEXT NOT NULL DEFAULT '[]',
            pass_end_ids TEXT NOT NULL DEFAULT '[]', pass_entrust_ids TEXT NOT NULL DEFAULT '[]',
            pass_collections TEXT NOT NULL DEFAULT '[]', last_game_time INTEGER NOT NULL DEFAULT 0,
            hotfix_str TEXT NOT NULL DEFAULT '', updated_at INTEGER NOT NULL DEFAULT 0
         );
         CREATE TABLE user_rouge2_materials (
            user_id INTEGER NOT NULL, material_id INTEGER NOT NULL,
            num INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (user_id, material_id)
         );",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO user_rouge2_state (user_id) VALUES (1)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO user_rouge2_materials (user_id, material_id, num)
         VALUES (1, 1000, 2), (1, 1101, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::raw_sql(include_str!(
        "../../../../migrations/103_rouge2_alchemy.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(
        sqlx::query_as::<_, (i32, i32, String)>(
            "SELECT alchemy_formula, alchemy_main_effect, alchemy_sub_effects
             FROM user_rouge2_state WHERE user_id = 1"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        (0, 0, "[]".into())
    );
    assert_eq!(
        sqlx::query_as::<_, (i32, bool)>(
            "SELECT material_id, is_seen FROM user_rouge2_materials ORDER BY material_id"
        )
        .fetch_all(&pool)
        .await
        .unwrap(),
        vec![(1000, true), (1101, false)]
    );
}

#[tokio::test]
async fn difficulty_transition_rolls_back_state_and_attributes_together() {
    let pool = test_pool().await;
    sqlx::query(
        "INSERT INTO user_rouge2_attrs
         (user_id, attr_id, final_val, updated_at) VALUES (1, 99, 9, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let result = apply_difficulty(
        &pool,
        1,
        DifficultyUpdate {
            difficulty: 1,
            attrs: &[(101, 1), (101, 2)],
        },
    )
    .await;

    assert!(result.is_err());
    assert_eq!(
        sqlx::query_as::<_, (i32, i32, i32)>(
            "SELECT state, difficulty, game_num FROM user_rouge2_state WHERE user_id = 1"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        (0, 0, 0)
    );
    assert_eq!(
        sqlx::query_as::<_, (i32, i32)>(
            "SELECT attr_id, final_val FROM user_rouge2_attrs WHERE user_id = 1"
        )
        .fetch_all(&pool)
        .await
        .unwrap(),
        vec![(99, 9)]
    );
}

#[tokio::test]
async fn run_attribute_migration_preserves_existing_rouge_state() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql(
        "CREATE TABLE users (id INTEGER PRIMARY KEY);
         CREATE TABLE user_rouge2_state (
            user_id INTEGER PRIMARY KEY,
            state INTEGER NOT NULL DEFAULT 0,
            game_num INTEGER NOT NULL DEFAULT 0
         );
         INSERT INTO users (id) VALUES (1);
         INSERT INTO user_rouge2_state (user_id, state, game_num) VALUES (1, 3, 7);",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::raw_sql(include_str!(
        "../../../../migrations/104_rouge2_run_attributes.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_rouge2_attrs
         (user_id, attr_id, final_val, updated_at) VALUES (1, 320, 1, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(
        sqlx::query_as::<_, (i32, i32)>(
            "SELECT state, game_num FROM user_rouge2_state WHERE user_id = 1"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        (3, 7)
    );
    assert_eq!(
        sqlx::query_as::<_, (i32, i32)>(
            "SELECT attr_id, final_val FROM user_rouge2_attrs WHERE user_id = 1"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        (320, 1)
    );
}
