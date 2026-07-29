use crate::models::game::bgm::{UserBgm, UserBgmState};
use anyhow::{Result, ensure};
use sonettobuf::BgmInfo;
use sqlx::SqlitePool;

async fn load_bgms(pool: &SqlitePool, player_id: i64) -> Result<Vec<UserBgm>> {
    Ok(sqlx::query_as::<_, UserBgm>(
        r#"
            SELECT
                player_id,
                bgm_id,
                unlock_time,
                is_favorite,
                is_read
            FROM user_bgm
            WHERE player_id = ?
            ORDER BY unlock_time
            "#,
    )
    .bind(player_id)
    .fetch_all(pool)
    .await?)
}

async fn load_bgm_state(pool: &SqlitePool, player_id: i64) -> Result<Option<UserBgmState>> {
    Ok(sqlx::query_as::<_, UserBgmState>(
        r#"
            SELECT player_id, use_bgm_id
            FROM user_bgm_state
            WHERE player_id = ?
            "#,
    )
    .bind(player_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn load_user_bgm(
    pool: &SqlitePool,
    player_id: i64,
) -> Result<(Vec<BgmInfo>, Option<i32>)> {
    let bgms = load_bgms(pool, player_id).await?;
    let state = load_bgm_state(pool, player_id).await?;

    Ok((
        bgms.into_iter().map(Into::into).collect(),
        state.map(|s| s.use_bgm_id),
    ))
}

pub async fn unlock_bgms(
    pool: &SqlitePool,
    player_id: i64,
    bgm_ids: &[i32],
    unlock_time: i32,
) -> Result<Vec<BgmInfo>> {
    let mut tx = pool.begin().await?;
    let mut unlocked = Vec::new();

    for &bgm_id in bgm_ids {
        let result = sqlx::query(
            "INSERT OR IGNORE INTO user_bgm
             (player_id, bgm_id, unlock_time, is_favorite, is_read)
             VALUES (?, ?, ?, 0, 0)",
        )
        .bind(player_id)
        .bind(bgm_id)
        .bind(unlock_time)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() != 0 {
            unlocked.push(BgmInfo {
                bgm_id: Some(bgm_id),
                unlock_time: Some(unlock_time),
                favorite: Some(false),
                is_read: Some(false),
            });
        }
    }

    tx.commit().await?;
    Ok(unlocked)
}

pub async fn set_active_bgm(pool: &SqlitePool, player_id: i64, bgm_id: i32) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;

    if bgm_id != 0 {
        let owns_bgm = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM user_bgm WHERE player_id = ? AND bgm_id = ?)",
        )
        .bind(player_id)
        .bind(bgm_id)
        .fetch_one(&mut *tx)
        .await?;
        ensure!(owns_bgm, "bgm {bgm_id} is not unlocked");
    }

    sqlx::query(
        r#"
        INSERT INTO user_bgm_state (player_id, use_bgm_id)
        VALUES (?, ?)
        ON CONFLICT(player_id)
        DO UPDATE SET use_bgm_id = excluded.use_bgm_id
        "#,
    )
    .bind(player_id)
    .bind(bgm_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn mark_bgm_read(pool: &SqlitePool, player_id: i64, bgm_id: i32) -> Result<()> {
    let result = sqlx::query("UPDATE user_bgm SET is_read = 1 WHERE player_id = ? AND bgm_id = ?")
        .bind(player_id)
        .bind(bgm_id)
        .execute(pool)
        .await?;
    ensure!(result.rows_affected() != 0, "bgm {bgm_id} is not unlocked");

    Ok(())
}

pub async fn set_bgm_favorite(
    pool: &SqlitePool,
    player_id: i64,
    bgm_id: i32,
    favorite: bool,
) -> anyhow::Result<()> {
    let result = sqlx::query(
        r#"
        UPDATE user_bgm
        SET is_favorite = ?, is_read = 1
        WHERE player_id = ? AND bgm_id = ?
        "#,
    )
    .bind(favorite)
    .bind(player_id)
    .bind(bgm_id)
    .execute(pool)
    .await?;
    ensure!(result.rows_affected() != 0, "bgm {bgm_id} is not unlocked");

    Ok(())
}
