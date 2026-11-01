use sqlx::{migrate::{MigrateError, Migrator}, sqlite::SqlitePoolOptions};
use std::{borrow::Cow, path::Path};

#[tokio::test]
async fn tts13_matching_services_share_migration_101_and_preserve_saved_data() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let current = Migrator::new(path.as_path()).await.unwrap();
    assert!(current.iter().any(|migration| migration.version == 101));
    let old_sdk = Migrator {
        migrations: Cow::Owned(current.iter().filter(|migration| migration.version <= 100)
            .cloned().collect()),
        ..Migrator::DEFAULT
    };
    let pool = SqlitePoolOptions::new().max_connections(1)
        .connect("sqlite::memory:").await.unwrap();
    old_sdk.run(&pool).await.unwrap();
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (920, 'tts13-test', 0, 0)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO items (user_id, item_id, quantity) VALUES (920, 620102, 77)")
        .execute(&pool).await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    assert!(matches!(old_sdk.run(&pool).await, Err(MigrateError::VersionMissing(101))));
    database::run_migrations(&pool).await.unwrap();
    let quantity: i32 = sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = 920 AND item_id = 620102")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(quantity, 77);
    let version: i64 = sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(version, 101);
}
