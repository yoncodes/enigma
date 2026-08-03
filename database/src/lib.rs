use sqlx::{Connection, Row, SqliteConnection, migrate, migrate::MigrateError};

use std::{collections::HashMap, path::Path};

use tracing::{info, warn};

mod config;
pub mod db;
pub mod models;

pub use config::DatabaseSettings;
pub use sqlx::{Error, SqlitePool, query, query_as};

pub async fn connect_to(settings: &DatabaseSettings) -> sqlx::Result<SqlitePool> {
    ensure_database_exists(&settings.db_name)?;

    SqlitePool::connect(&settings.to_string()).await
}

pub async fn run_migrations(pool: &SqlitePool) -> Result<(), migrate::MigrateError> {
    info!("Running database migrations...");
    migrate!("./migrations").run(pool).await?;
    info!("Migrations completed successfully");
    Ok(())
}

/// Runs migrations, and if a checksum mismatch is detected (because a migration file was edited
/// in-place), backs up all user data, recreates the DB from scratch, and restores the data.
/// New columns introduced by the edited migration get their DEFAULT values automatically.
pub async fn migrate_or_rescue(settings: &DatabaseSettings) -> anyhow::Result<SqlitePool> {
    let pool = connect_to(settings).await?;

    match run_migrations(&pool).await {
        Ok(()) => return Ok(pool),
        Err(MigrateError::VersionMismatch(v)) => {
            warn!("Migration checksum mismatch at version {v} — starting DB rescue");
        }
        Err(MigrateError::VersionMissing(v)) => {
            warn!("Migration version {v} no longer exists — starting DB rescue");
        }
        Err(e) => return Err(e.into()),
    }

    let backup = dump_all_tables(&pool).await?;
    pool.close().await;

    let bak_path = format!("{}.bak", settings.db_name);
    std::fs::rename(&settings.db_name, &bak_path)
        .map_err(|e| anyhow::anyhow!("Failed to rename DB to .bak: {e}"))?;
    info!("Old DB saved as {bak_path}, recreating schema...");

    let pool = reinstate_on_error(
        connect_to(settings).await.map_err(anyhow::Error::from),
        &settings.db_name,
        &bak_path,
    )?;
    let rescue_result: anyhow::Result<()> = async {
        run_migrations(&pool).await?;
        restore_all_tables(&pool, backup).await
    }
    .await;
    if let Err(error) = rescue_result {
        pool.close().await;
        return reinstate_on_error(Err(error), &settings.db_name, &bak_path);
    }
    info!("DB rescue complete");

    Ok(pool)
}

fn reinstate_backup(db_path: &str, bak_path: &str) -> std::io::Result<()> {
    if Path::new(db_path).exists() {
        std::fs::remove_file(db_path)?;
    }
    std::fs::rename(bak_path, db_path)
}

fn reinstate_on_error<T>(
    result: anyhow::Result<T>,
    db_path: &str,
    bak_path: &str,
) -> anyhow::Result<T> {
    result.map_err(|error| match reinstate_backup(db_path, bak_path) {
        Ok(()) => error,
        Err(restore_error) => anyhow::anyhow!(
            "DB rescue failed: {error}; failed to reinstate {bak_path}: {restore_error}"
        ),
    })
}

#[derive(Debug, Clone)]
enum SqlVal {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

// table name → rows, each row is ordered (col_name, value) pairs
type Dump = HashMap<String, Vec<Vec<(String, SqlVal)>>>;

async fn dump_all_tables(pool: &SqlitePool) -> anyhow::Result<Dump> {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master
         WHERE type = 'table'
           AND name NOT LIKE 'sqlite_%'
           AND name NOT LIKE '_sqlx_%'
         ORDER BY name",
    )
    .fetch_all(pool)
    .await?;

    let mut dump: Dump = HashMap::new();

    for table in &tables {
        // PRAGMA table_info returns: cid(i32), name(String), type(String),
        //                            notnull(i32), dflt_value(Option<String>), pk(i32)
        let col_types: Vec<(i32, String, String, i32, Option<String>, i32)> =
            sqlx::query_as(&format!("PRAGMA table_info(\"{}\")", table))
                .fetch_all(pool)
                .await?;
        // Keep only (name, declared_type) pairs
        let col_types: Vec<(String, String)> = col_types
            .into_iter()
            .map(|(_, name, typ, _, _, _)| (name, typ))
            .collect();

        let rows = sqlx::query(&format!("SELECT * FROM \"{}\"", table))
            .fetch_all(pool)
            .await?;

        let mut table_rows = Vec::with_capacity(rows.len());

        for row in &rows {
            let mut cols = Vec::with_capacity(col_types.len());
            for (col_name, col_type) in &col_types {
                let idx = col_name.as_str();
                let affinity = col_type.to_uppercase();

                let val = if affinity.contains("INT") || affinity.contains("BOOL") {
                    match row.try_get::<Option<i64>, _>(idx) {
                        Ok(Some(v)) => SqlVal::Int(v),
                        Ok(None) => SqlVal::Null,
                        Err(_) => decode_fallback(row, idx),
                    }
                } else if affinity.contains("REAL")
                    || affinity.contains("FLOA")
                    || affinity.contains("DOUB")
                {
                    match row.try_get::<Option<f64>, _>(idx) {
                        Ok(Some(v)) => SqlVal::Real(v),
                        Ok(None) => SqlVal::Null,
                        Err(_) => decode_fallback(row, idx),
                    }
                } else if affinity.contains("BLOB") {
                    match row.try_get::<Option<Vec<u8>>, _>(idx) {
                        Ok(Some(v)) => SqlVal::Blob(v),
                        Ok(None) => SqlVal::Null,
                        Err(_) => decode_fallback(row, idx),
                    }
                } else {
                    // TEXT / VARCHAR / JSON / anything else
                    match row.try_get::<Option<String>, _>(idx) {
                        Ok(Some(v)) => SqlVal::Text(v),
                        Ok(None) => SqlVal::Null,
                        Err(_) => decode_fallback(row, idx),
                    }
                };

                cols.push((col_name.clone(), val));
            }
            table_rows.push(cols);
        }

        info!("Backed up {} rows from `{}`", table_rows.len(), table);
        dump.insert(table.clone(), table_rows);
    }

    Ok(dump)
}

fn decode_fallback(row: &sqlx::sqlite::SqliteRow, col: &str) -> SqlVal {
    if let Ok(Some(v)) = row.try_get::<Option<i64>, _>(col) {
        return SqlVal::Int(v);
    }
    if let Ok(Some(v)) = row.try_get::<Option<f64>, _>(col) {
        return SqlVal::Real(v);
    }
    if let Ok(Some(v)) = row.try_get::<Option<String>, _>(col) {
        return SqlVal::Text(v);
    }
    if let Ok(Some(v)) = row.try_get::<Option<Vec<u8>>, _>(col) {
        return SqlVal::Blob(v);
    }
    SqlVal::Null
}

async fn restore_all_tables(pool: &SqlitePool, dump: Dump) -> anyhow::Result<()> {
    let mut conn = pool.acquire().await?;

    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *conn)
        .await?;

    let restore_result: anyhow::Result<()> = async {
        let mut transaction = conn.begin().await?;

        for (table, rows) in &dump {
            if table == "user_activity101_claims" {
                let restored = restore_legacy_activity101_claims(&mut transaction, rows).await?;
                info!("Migrated `{table}` into `user_activity_state`: {restored} rows");
                continue;
            }

            if table == "user_activity101_once_bonus" {
                let restored =
                    restore_legacy_activity101_once_bonus(&mut transaction, rows).await?;
                info!("Migrated `{table}` into `user_activity_state`: {restored} rows");
                continue;
            }

            if rows.is_empty() {
                continue;
            }

            let mut restored = 0usize;
            for row in rows {
                if row.is_empty() {
                    continue;
                }

                let col_list = row
                    .iter()
                    .map(|(c, _)| format!("\"{}\"", c))
                    .collect::<Vec<_>>()
                    .join(", ");
                let placeholders = row.iter().map(|_| "?").collect::<Vec<_>>().join(", ");

                let sql = format!(
                    "INSERT INTO \"{}\" ({}) VALUES ({})",
                    table, col_list, placeholders
                );

                let mut q = sqlx::query(&sql);
                for (_, val) in row {
                    q = match val {
                        SqlVal::Null => q.bind(None::<i64>),
                        SqlVal::Int(v) => q.bind(v),
                        SqlVal::Real(v) => q.bind(v),
                        SqlVal::Text(v) => q.bind(v),
                        SqlVal::Blob(v) => q.bind(v.as_slice()),
                    };
                }

                let result = q.execute(&mut *transaction).await?;
                if result.rows_affected() != 1 {
                    anyhow::bail!("Restoring `{table}` changed no rows");
                }
                restored += 1;
            }

            info!("Restored `{table}`: {restored} rows");
        }

        if sqlx::query("PRAGMA foreign_key_check")
            .fetch_optional(&mut *transaction)
            .await?
            .is_some()
        {
            anyhow::bail!("Restored database contains foreign-key violations");
        }

        transaction.commit().await?;
        Ok(())
    }
    .await;

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *conn)
        .await?;

    restore_result
}

async fn restore_legacy_activity101_claims(
    conn: &mut SqliteConnection,
    rows: &[Vec<(String, SqlVal)>],
) -> anyhow::Result<usize> {
    let mut restored = 0usize;
    for row in rows {
        let user_id = required_int_col(row, "user_id")?;
        let activity_id = required_int_col(row, "activity_id")?;
        let day_id = required_int_col(row, "day_id")?;
        let Some(claimed_at) = required_nullable_int_col(row, "claimed_at")? else {
            continue;
        };

        sqlx::query(
            "INSERT INTO user_activity_state
                (user_id, activity_id, kind, entry_id, state, progress, ext, updated_at)
             VALUES (?, ?, 1, ?, 2, 0, '', ?)
             ON CONFLICT(user_id, activity_id, kind, entry_id)
             DO UPDATE SET
                state = excluded.state,
                updated_at = excluded.updated_at",
        )
        .bind(user_id)
        .bind(activity_id)
        .bind(day_id)
        .bind(claimed_at)
        .execute(&mut *conn)
        .await?;

        restored += 1;
    }
    Ok(restored)
}

async fn restore_legacy_activity101_once_bonus(
    conn: &mut SqliteConnection,
    rows: &[Vec<(String, SqlVal)>],
) -> anyhow::Result<usize> {
    let mut restored = 0usize;
    for row in rows {
        let user_id = required_int_col(row, "user_id")?;
        let activity_id = required_int_col(row, "activity_id")?;
        let Some(claimed_at) = required_nullable_int_col(row, "claimed_at")? else {
            continue;
        };

        sqlx::query(
            "INSERT INTO user_activity_state
                (user_id, activity_id, kind, entry_id, state, progress, ext, updated_at)
             VALUES (?, ?, 2, 0, 2, 0, '', ?)
             ON CONFLICT(user_id, activity_id, kind, entry_id)
             DO UPDATE SET
                state = excluded.state,
                updated_at = excluded.updated_at",
        )
        .bind(user_id)
        .bind(activity_id)
        .bind(claimed_at)
        .execute(&mut *conn)
        .await?;

        restored += 1;
    }
    Ok(restored)
}

fn int_col(row: &[(String, SqlVal)], name: &str) -> Option<i64> {
    row.iter()
        .find(|(col, _)| col == name)
        .and_then(|(_, val)| match val {
            SqlVal::Int(value) => Some(*value),
            _ => None,
        })
}

fn required_int_col(row: &[(String, SqlVal)], name: &str) -> anyhow::Result<i64> {
    int_col(row, name).ok_or_else(|| anyhow::anyhow!("Legacy row has no integer `{name}` column"))
}

fn required_nullable_int_col(row: &[(String, SqlVal)], name: &str) -> anyhow::Result<Option<i64>> {
    match row.iter().find(|(col, _)| col == name) {
        Some((_, SqlVal::Int(value))) => Ok(Some(*value)),
        Some((_, SqlVal::Null)) => Ok(None),
        Some(_) => anyhow::bail!("Legacy row has non-integer `{name}` column"),
        None => anyhow::bail!("Legacy row has no `{name}` column"),
    }
}

fn ensure_database_exists(db_path: &str) -> sqlx::Result<()> {
    let path = Path::new(db_path);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(Error::Io)?;
        info!("Ensured database directory exists: {}", parent.display());
    }

    if !path.exists() {
        std::fs::File::create(path).map_err(Error::Io)?;
        info!("Created new database file: {}", db_path);
    } else {
        info!("Using existing database: {}", db_path);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
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
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM user_activity_state WHERE entry_id = 4"
            )
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
}
