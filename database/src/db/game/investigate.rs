use anyhow::Result;
use sqlx::SqlitePool;

pub async fn get_linked_clues(pool: &SqlitePool, user_id: i64) -> Result<Vec<(i32, i32)>> {
    Ok(sqlx::query_as(
        "SELECT info_id, clue_id
         FROM user_investigate_clues
         WHERE user_id = ?
         ORDER BY info_id, clue_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_dungeon_element_ids(pool: &SqlitePool, user_id: i64) -> Result<Vec<i32>> {
    Ok(sqlx::query_scalar(
        "SELECT element_id
         FROM user_dungeon_elements
         WHERE user_id = ?
         ORDER BY element_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?)
}

pub async fn link_clue(pool: &SqlitePool, user_id: i64, info_id: i32, clue_id: i32) -> Result<()> {
    sqlx::query(
        "INSERT INTO user_investigate_clues (user_id, info_id, clue_id)
         VALUES (?, ?, ?)
         ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(info_id)
    .bind(clue_id)
    .execute(pool)
    .await?;

    Ok(())
}
