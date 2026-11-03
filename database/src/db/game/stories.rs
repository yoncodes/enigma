use crate::models::game::stories::{HeroStoryState, ProcessingStory};
use anyhow::Result;
use sqlx::{QueryBuilder, Sqlite, SqlitePool, Transaction};
pub async fn get_finished_stories(pool: &SqlitePool, user_id: i64) -> Result<Vec<i32>> {
    let stories = sqlx::query_scalar(
        "SELECT story_id FROM user_finished_stories WHERE user_id = ? ORDER BY story_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(stories)
}

pub async fn get_processing_stories(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<Vec<ProcessingStory>> {
    let stories = sqlx::query_as::<_, ProcessingStory>(
        "SELECT * FROM user_processing_stories WHERE user_id = ? ORDER BY story_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(stories)
}

pub async fn finish_story(pool: &SqlitePool, user_id: i64, story_id: i32) -> Result<()> {
    let mut tx = pool.begin().await?;
    finish_story_in_transaction(&mut tx, user_id, story_id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn finish_story_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    story_id: i32,
) -> Result<bool> {
    // Move from processing to finished
    let deleted =
        sqlx::query("DELETE FROM user_processing_stories WHERE user_id = ? AND story_id = ?")
            .bind(user_id)
            .bind(story_id)
            .execute(&mut **tx)
            .await?;

    let inserted = sqlx::query("INSERT INTO user_finished_stories (user_id, story_id) VALUES (?, ?) ON CONFLICT DO NOTHING")
        .bind(user_id)
        .bind(story_id)
        .execute(&mut **tx)
        .await?;

    Ok(deleted.rows_affected() != 0 || inserted.rows_affected() != 0)
}

pub async fn update_processing_story(
    pool: &SqlitePool,
    user_id: i64,
    story_id: i32,
    step_id: i32,
    favor: i32,
) -> Result<()> {
    let now = common::time::ServerTime::now_ms();

    sqlx::query(
        r#"
        INSERT INTO user_processing_stories (user_id, story_id, step_id, favor, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?)
        ON CONFLICT(user_id, story_id) DO UPDATE SET
            step_id = excluded.step_id,
            favor = excluded.favor,
            updated_at = excluded.updated_at
        "#
    )
    .bind(user_id)
    .bind(story_id)
    .bind(step_id)
    .bind(favor)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn is_story_finished(pool: &SqlitePool, user_id: i64, story_id: i32) -> Result<bool> {
    let finished = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM user_finished_stories WHERE user_id = ? AND story_id = ?)",
    )
    .bind(user_id)
    .bind(story_id)
    .fetch_one(pool)
    .await?;

    Ok(finished)
}

pub async fn get_hero_story_states(pool: &SqlitePool, user_id: i64) -> Result<Vec<HeroStoryState>> {
    let states = sqlx::query_as::<_, HeroStoryState>(
        "SELECT user_id, story_id, progress, get_reward, get_score_bonus, score,
                challenge_wave, challenge_max_wave, get_challenge_reward, unlock,
                is_new, updated_at
         FROM user_hero_story_state
         WHERE user_id = ?
         ORDER BY story_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(states)
}

pub async fn is_hero_story_unlocked(
    pool: &SqlitePool,
    user_id: i64,
    story_id: i32,
) -> Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM user_hero_story_state
            WHERE user_id = ? AND story_id = ? AND unlock = 1
        )",
    )
    .bind(user_id)
    .bind(story_id)
    .fetch_one(pool)
    .await?)
}

pub async fn unlock_hero_story(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    story_id: i32,
) -> Result<Option<HeroStoryState>> {
    let state = sqlx::query_as::<_, HeroStoryState>(
        "UPDATE user_hero_story_state
         SET unlock = 1, updated_at = ?
         WHERE user_id = ? AND story_id = ? AND unlock = 0
         RETURNING user_id, story_id, progress, get_reward, get_score_bonus, score,
                   challenge_wave, challenge_max_wave, get_challenge_reward, unlock,
                   is_new, updated_at",
    )
    .bind(common::time::ServerTime::now_ms())
    .bind(user_id)
    .bind(story_id)
    .fetch_optional(&mut **tx)
    .await?;

    Ok(state)
}

pub async fn sync_hero_story_states(
    pool: &SqlitePool,
    user_id: i64,
    tables: &config::GameDB,
) -> Result<()> {
    let now = common::time::ServerTime::now_ms();
    let existing: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM user_hero_story_state WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(pool)
            .await?;

    if existing >= tables.hero_story.len() as i64 {
        return Ok(());
    }

    for chunk in tables.hero_story.all().chunks(200) {
        let mut query = QueryBuilder::<Sqlite>::new(
            "INSERT INTO user_hero_story_state (user_id, story_id, unlock, updated_at) ",
        );

        query.push_values(chunk, |mut row, story| {
            row.push_bind(user_id)
                .push_bind(story.id)
                .push_bind(story.unlock.is_empty() && story.permanent_unlock.is_empty())
                .push_bind(now);
        });

        query.push(" ON CONFLICT DO NOTHING");
        query.build().execute(pool).await?;
    }

    Ok(())
}
