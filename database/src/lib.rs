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
mod tests;
