use sqlx::{migrate::{MigrateError, Migrator}, sqlite::SqlitePoolOptions};
use std::{borrow::Cow, path::Path};

#[tokio::test]
async fn newer_game_and_matching_sdk_share_migrations_without_rebuilding_user_data() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let latest = Migrator::new(path.as_path()).await.unwrap();
    let old = Migrator {
        migrations: Cow::Owned(latest.iter().filter(|migration| migration.version <= 96)
            .cloned().collect()),
        ..Migrator::DEFAULT
    };
    let pool = SqlitePoolOptions::new().max_connections(1)
        .connect("sqlite::memory:").await.unwrap();
    old.run(&pool).await.unwrap();
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (919, 'isolated-user', 0, 0)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO items (user_id, item_id, quantity) VALUES (919, 620102, 77)")
        .execute(&pool).await.unwrap();

    // The new game service upgrades the old test database.
    database::run_migrations(&pool).await.unwrap();
    // An old SDK then sees the exact version-missing failure from the log.
    assert!(matches!(old.run(&pool).await, Err(MigrateError::VersionMissing(97))));
    // A matching SDK uses the same embedded migrations and can open it again.
    database::run_migrations(&pool).await.unwrap();
    let quantity: i32 = sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = 919 AND item_id = 620102")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(quantity, 77);
    let version: i64 = sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(version, 100);
}
