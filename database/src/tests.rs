use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

#[tokio::test]
async fn migrations_run_on_fresh_database() {
    let name = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("enigma-db-migrations-{name}"));
    let db_path = dir.join("sonetto.db");

    let pool = connect_to(&DatabaseSettings {
        db_name: db_path.to_string_lossy().to_string(),
    })
    .await
    .unwrap();

    run_migrations(&pool).await.unwrap();
    pool.close().await;

    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn rescue_migrates_legacy_activity101_claims() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(
        "CREATE TABLE user_activity_state (
                user_id INTEGER NOT NULL,
                activity_id INTEGER NOT NULL,
                kind INTEGER NOT NULL,
                entry_id INTEGER NOT NULL,
                state INTEGER NOT NULL DEFAULT 0,
                progress INTEGER NOT NULL DEFAULT 0,
                ext TEXT NOT NULL DEFAULT '',
                updated_at INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (user_id, activity_id, kind, entry_id)
            )",
    )
    .execute(&pool)
    .await
    .unwrap();

    let rows = vec![
        vec![
            ("user_id".to_string(), SqlVal::Int(7)),
            ("activity_id".to_string(), SqlVal::Int(13108)),
            ("day_id".to_string(), SqlVal::Int(3)),
            ("claimed_at".to_string(), SqlVal::Int(1234)),
        ],
        vec![
            ("user_id".to_string(), SqlVal::Int(7)),
            ("activity_id".to_string(), SqlVal::Int(13108)),
            ("day_id".to_string(), SqlVal::Int(4)),
            ("claimed_at".to_string(), SqlVal::Null),
        ],
    ];

    let mut conn = pool.acquire().await.unwrap();
    let restored = restore_legacy_activity101_claims(&mut conn, &rows)
        .await
        .unwrap();

    let state: (i32, i32) = sqlx::query_as(
        "SELECT state, updated_at
             FROM user_activity_state
             WHERE user_id = 7 AND activity_id = 13108 AND kind = 1 AND entry_id = 3",
    )
    .fetch_one(&mut *conn)
    .await
    .unwrap();

    assert_eq!(restored, 1);
    assert_eq!(state, (2, 1234));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM user_activity_state WHERE entry_id = 4")
            .fetch_one(&mut *conn)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn rescue_restores_rows_and_reenables_foreign_keys() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE restored_items (id INTEGER PRIMARY KEY, value TEXT NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();

    let dump = HashMap::from([(
        "restored_items".to_string(),
        vec![
            vec![
                ("id".to_string(), SqlVal::Int(1)),
                ("value".to_string(), SqlVal::Text("first".to_string())),
            ],
            vec![
                ("id".to_string(), SqlVal::Int(2)),
                ("value".to_string(), SqlVal::Text("second".to_string())),
            ],
        ],
    )]);

    restore_all_tables(&pool, dump).await.unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM restored_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(count, 2);
    assert_eq!(foreign_keys, 1);
}

#[tokio::test]
async fn rescue_rejects_partial_table_restore() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE restored_items (id INTEGER PRIMARY KEY, value TEXT NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();
    let row = |value: &str| {
        vec![
            ("id".to_string(), SqlVal::Int(1)),
            ("value".to_string(), SqlVal::Text(value.to_string())),
        ]
    };

    let result = restore_all_tables(
        &pool,
        HashMap::from([(
            "restored_items".to_string(),
            vec![row("first"), row("duplicate")],
        )]),
    )
    .await;

    assert!(result.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM restored_items")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn rescue_rejects_malformed_legacy_rows() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(
        "CREATE TABLE user_activity_state (
                user_id INTEGER NOT NULL,
                activity_id INTEGER NOT NULL,
                kind INTEGER NOT NULL,
                entry_id INTEGER NOT NULL,
                state INTEGER NOT NULL DEFAULT 0,
                progress INTEGER NOT NULL DEFAULT 0,
                ext TEXT NOT NULL DEFAULT '',
                updated_at INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (user_id, activity_id, kind, entry_id)
            )",
    )
    .execute(&pool)
    .await
    .unwrap();
    let dump = HashMap::from([(
        "user_activity101_claims".to_string(),
        vec![vec![("user_id".to_string(), SqlVal::Int(7))]],
    )]);

    assert!(restore_all_tables(&pool, dump).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM user_activity_state")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn rescue_rejects_foreign_key_violations() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE parents (id INTEGER PRIMARY KEY)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE children (id INTEGER PRIMARY KEY, parent_id INTEGER NOT NULL REFERENCES parents(id))",
    )
    .execute(&pool)
    .await
    .unwrap();
    let dump = HashMap::from([(
        "children".to_string(),
        vec![vec![
            ("id".to_string(), SqlVal::Int(1)),
            ("parent_id".to_string(), SqlVal::Int(99)),
        ]],
    )]);

    assert!(restore_all_tables(&pool, dump).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM children")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[test]
fn recreate_failure_reinstates_the_backup() {
    let name = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("enigma-db-open-failure-{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("sonetto.db");
    let bak_path = dir.join("sonetto.db.bak");
    std::fs::write(&db_path, b"partial replacement").unwrap();
    std::fs::write(&bak_path, b"original database").unwrap();

    assert!(
        reinstate_on_error::<()>(
            Err(anyhow::anyhow!("replacement open failed")),
            db_path.to_str().unwrap(),
            bak_path.to_str().unwrap(),
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&db_path).unwrap(), b"original database");
    assert!(!bak_path.exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn failed_rescue_reinstates_the_original_database() {
    let name = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("enigma-db-rescue-failure-{name}"));
    let db_path = dir.join("sonetto.db");
    let settings = DatabaseSettings {
        db_name: db_path.to_string_lossy().to_string(),
    };
    let pool = connect_to(&settings).await.unwrap();
    run_migrations(&pool).await.unwrap();
    sqlx::query("CREATE TABLE rescue_only (value TEXT NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO rescue_only (value) VALUES ('preserved')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE _sqlx_migrations SET checksum = X'00' WHERE version = (SELECT MIN(version) FROM _sqlx_migrations)")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    assert!(migrate_or_rescue(&settings).await.is_err());

    let restored = connect_to(&settings).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT value FROM rescue_only")
            .fetch_one(&restored)
            .await
            .unwrap(),
        "preserved"
    );
    restored.close().await;
    assert!(!Path::new(&format!("{}.bak", settings.db_name)).exists());
    std::fs::remove_dir_all(dir).unwrap();
}
