use anyhow::{Context, Result, ensure};
use sonettobuf::InstructionDungeonInfoReply;
use sqlx::SqlitePool;
use std::collections::HashSet;

pub async fn reconcile_unlocks(pool: &SqlitePool, user_id: i64) -> Result<bool> {
    let completed = sqlx::query_scalar::<_, i32>(
        "SELECT episode_id FROM user_dungeons WHERE user_id = ? AND star > 0",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect::<HashSet<_>>();
    let existing = sqlx::query_scalar::<_, i32>(
        "SELECT instruction_id FROM user_instruction_dungeon_unlocks WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect::<HashSet<_>>();
    let mut changed = false;

    for level in config::configs::get()
        .instruction_level
        .iter()
        .filter(|level| level.pre_episode == 0 || completed.contains(&level.pre_episode))
    {
        if existing.contains(&level.episode_id) {
            continue;
        }
        sqlx::query(
            "INSERT INTO user_instruction_dungeon_unlocks (user_id, instruction_id)
             VALUES (?, ?) ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(level.episode_id)
        .execute(pool)
        .await?;
        changed = true;
    }

    Ok(changed)
}

pub async fn get_info(pool: &SqlitePool, user_id: i64) -> Result<InstructionDungeonInfoReply> {
    let unlock_ids = sqlx::query_scalar(
        "SELECT instruction_id FROM user_instruction_dungeon_unlocks WHERE user_id = ? ORDER BY instruction_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let get_reward_ids = sqlx::query_scalar(
        "SELECT reward_id FROM user_instruction_dungeon_rewards WHERE user_id = ? ORDER BY reward_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let open_ids = sqlx::query_scalar(
        "SELECT instruction_id FROM user_instruction_dungeon_opens WHERE user_id = ? ORDER BY instruction_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let get_final_reward = sqlx::query_scalar(
        "SELECT get_final_reward FROM user_instruction_dungeon_state WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .unwrap_or(false);

    Ok(InstructionDungeonInfoReply {
        unlock_ids,
        get_reward_ids,
        get_final_reward: Some(get_final_reward),
        open_ids,
    })
}

pub async fn add_open_ids(pool: &SqlitePool, user_id: i64, ids: Vec<i32>) -> Result<bool> {
    reconcile_unlocks(pool, user_id).await?;
    let unlocked = sqlx::query_scalar::<_, i32>(
        "SELECT instruction_id FROM user_instruction_dungeon_unlocks WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect::<HashSet<_>>();
    let mut changed = false;

    for id in ids {
        ensure!(unlocked.contains(&id), "instruction episode {id} is locked");
        changed |= sqlx::query(
            "INSERT INTO user_instruction_dungeon_opens (user_id, instruction_id) VALUES (?, ?) ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected()
            != 0;
    }

    Ok(changed)
}

pub async fn claim_topic_reward(pool: &SqlitePool, user_id: i64, topic_id: i32) -> Result<bool> {
    let mut tx = pool.begin().await?;
    let claimed = claim_topic_reward_in_transaction(&mut tx, user_id, topic_id).await?;
    tx.commit().await?;
    Ok(claimed)
}

pub async fn claim_topic_reward_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
    topic_id: i32,
) -> Result<bool> {
    let game_data = config::configs::get();
    game_data
        .instruction_topic
        .get(topic_id)
        .with_context(|| format!("missing instruction topic {topic_id}"))?;

    let completed = sqlx::query_scalar::<_, i32>(
        "SELECT episode_id FROM user_dungeons WHERE user_id = ? AND star > 0",
    )
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .collect::<HashSet<_>>();
    ensure!(
        game_data
            .instruction_level
            .iter()
            .filter(|level| level.topic_id == topic_id)
            .all(|level| completed.contains(&level.episode_id)),
        "instruction topic {topic_id} is incomplete"
    );

    let result = sqlx::query(
        "INSERT INTO user_instruction_dungeon_rewards (user_id, reward_id) VALUES (?, ?) ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(topic_id)
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected() != 0)
}

pub async fn claim_final_reward(pool: &SqlitePool, user_id: i64) -> Result<bool> {
    let mut tx = pool.begin().await?;
    let claimed = claim_final_reward_in_transaction(&mut tx, user_id).await?;
    tx.commit().await?;
    Ok(claimed)
}

pub async fn claim_final_reward_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
) -> Result<bool> {
    let claimed_topics = sqlx::query_scalar::<_, i32>(
        "SELECT reward_id FROM user_instruction_dungeon_rewards WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .collect::<HashSet<_>>();
    ensure!(
        config::configs::get()
            .instruction_topic
            .iter()
            .all(|topic| claimed_topics.contains(&topic.id)),
        "instruction topic rewards are incomplete"
    );

    let result = sqlx::query(
        r#"
        INSERT INTO user_instruction_dungeon_state (user_id, get_final_reward)
        VALUES (?, 1)
        ON CONFLICT(user_id) DO UPDATE SET get_final_reward = 1
        WHERE get_final_reward = 0
        "#,
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected() != 0)
}
