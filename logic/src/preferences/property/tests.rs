use super::*;

#[tokio::test]
async fn set_simple_property_persists_value() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(
        "CREATE TABLE user_simple_properties (
                user_id INTEGER NOT NULL,
                property_id INTEGER NOT NULL,
                property_value TEXT NOT NULL,
                PRIMARY KEY(user_id, property_id)
            )",
    )
    .execute(&pool)
    .await
    .unwrap();

    PreferenceManager::new(7)
        .set_simple_property(&pool, 9, "on".to_string())
        .await
        .unwrap();
    let value: String = sqlx::query_scalar(
        "SELECT property_value FROM user_simple_properties WHERE user_id = 7 AND property_id = 9",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(value, "on");
}

#[tokio::test]
async fn main_scene_skin_requires_its_configured_item() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (8, 'scene', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let preferences = PreferenceManager::new(8);
    preferences
        .set_main_scene_skin(&pool, config::configs::get(), 0)
        .await
        .unwrap();
    assert!(
        preferences
            .set_main_scene_skin(&pool, config::configs::get(), 240002)
            .await
            .is_err()
    );
    items::add_item_quantity(&pool, 8, 240002, 1).await.unwrap();
    preferences
        .set_main_scene_skin(&pool, config::configs::get(), 240002)
        .await
        .unwrap();

    let value: String = sqlx::query_scalar(
        "SELECT property_value FROM user_simple_properties
             WHERE user_id = 8 AND property_id = ?",
    )
    .bind(MAIN_SCENE_SKIN_PROPERTY_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(value, "240002");
}

#[tokio::test]
async fn main_ui_skin_requires_its_configured_item() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (9, 'ui', 0, 0)")
        .execute(&pool)
        .await
        .unwrap();

    let preferences = PreferenceManager::new(9);
    preferences
        .set_ui_style_skin(&pool, config::configs::get(), 0)
        .await
        .unwrap();
    assert!(
        preferences
            .set_ui_style_skin(&pool, config::configs::get(), 350002)
            .await
            .is_err()
    );
    items::add_item_quantity(&pool, 9, 350002, 1).await.unwrap();
    preferences
        .set_ui_style_skin(&pool, config::configs::get(), 350002)
        .await
        .unwrap();

    let value: String = sqlx::query_scalar(
        "SELECT property_value FROM user_simple_properties
             WHERE user_id = 9 AND property_id = ?",
    )
    .bind(MAIN_UI_SKIN_PROPERTY_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(value, "350002");
}
