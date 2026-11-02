use anyhow::Result;
use common::time::ServerTime;
use sonettobuf::OpenInfo;
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug)]
pub struct OpenInfoSeed {
    pub id: i32,
    pub is_open: bool,
}

pub async fn ensure_open_infos(
    pool: &SqlitePool,
    user_id: i64,
    seeds: &[OpenInfoSeed],
) -> Result<()> {
    let now = ServerTime::now_ms();
    for seed in seeds {
        sqlx::query(
            "INSERT INTO user_open_infos (user_id, open_id, is_open, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(user_id, open_id) DO NOTHING",
        )
        .bind(user_id)
        .bind(seed.id)
        .bind(seed.is_open)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;
    }

    Ok(())
}

pub fn initial_state(open: &config::open::Open) -> bool {
    open.is_online != 0
        && open.player_lv <= 1
        && open.episode_id == 0
        && open.element_id == 0
        && open.room_level == 0
        && open.bind_activity_id == 0
        && open.daily_open_time.is_empty()
}

pub async fn reconcile_progression(pool: &SqlitePool, user_id: i64) -> Result<Vec<OpenInfo>> {
    let mut tx = pool.begin().await?;
    let changed = reconcile_progression_in_transaction(&mut tx, user_id).await?;
    tx.commit().await?;
    Ok(changed)
}

pub async fn reconcile_progression_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: i64,
) -> Result<Vec<OpenInfo>> {
    let player_level = sqlx::query_scalar::<_, i32>("SELECT level FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&mut **tx)
        .await?;
    let completed_episodes = sqlx::query_scalar::<_, i32>(
        "SELECT episode_id FROM user_dungeons WHERE user_id = ? AND star > 0",
    )
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .collect::<HashSet<_>>();
    // The client-facing episode and its progression-chain id may differ.
    // Treat an alias as completed only when its corresponding episode was cleared.
    let completed_episodes = completed_episodes_with_chain_aliases(
        &completed_episodes,
        config::configs::get()
            .episode
            .iter()
            .filter(|episode| episode.chain_episode > 0)
            .map(|episode| (episode.id, episode.chain_episode)),
    );
    let finished_elements = sqlx::query_scalar::<_, i32>(
        "SELECT element_id FROM user_dungeon_elements WHERE user_id = ? AND is_finished = 1",
    )
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .collect::<HashSet<_>>();
    let room_level = sqlx::query_scalar::<_, i32>(
        "SELECT COALESCE((SELECT room_level FROM user_room_state WHERE user_id = ?), 0)",
    )
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await?;
    let now = ServerTime::now_ms();
    let current = sqlx::query_as::<_, (i32, bool)>(
        "SELECT open_id, is_open FROM user_open_infos WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .collect::<HashMap<_, _>>();
    let mut changed = Vec::new();

    for open in config::configs::get().open.iter().filter(|open| {
        open.is_online == 0 || (open.bind_activity_id == 0 && open.daily_open_time.is_empty())
    }) {
        let is_open = open.is_online != 0
            && player_level >= open.player_lv
            && (open.episode_id == 0 || completed_episodes.contains(&open.episode_id))
            && (open.element_id == 0 || finished_elements.contains(&open.element_id))
            && (open.room_level == 0 || room_level >= open.room_level);
        if current.get(&open.id) == Some(&is_open) {
            continue;
        }
        let row = sqlx::query_as::<_, (i32, bool)>(
            "INSERT INTO user_open_infos (user_id, open_id, is_open, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(user_id, open_id) DO UPDATE SET
                is_open = excluded.is_open,
                updated_at = excluded.updated_at
             WHERE user_open_infos.is_open != excluded.is_open
             RETURNING open_id, is_open",
        )
        .bind(user_id)
        .bind(open.id)
        .bind(is_open)
        .bind(now)
        .bind(now)
        .fetch_optional(&mut **tx)
        .await?;

        if let Some((id, is_open)) = row {
            changed.push(OpenInfo { id, is_open });
        }
    }

    Ok(changed)
}

pub async fn list_open_infos(pool: &SqlitePool, user_id: i64) -> Result<Vec<OpenInfo>> {
    let rows = sqlx::query_as::<_, (i32, bool)>(
        "SELECT open_id, is_open FROM user_open_infos WHERE user_id = ? ORDER BY open_id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, is_open)| OpenInfo { id, is_open })
        .collect())
}

pub async fn get_open_info(pool: &SqlitePool, user_id: i64, open_id: i32) -> Result<OpenInfo> {
    let row = sqlx::query_as::<_, (i32, bool)>(
        "SELECT open_id, is_open FROM user_open_infos WHERE user_id = ? AND open_id = ?",
    )
    .bind(user_id)
    .bind(open_id)
    .fetch_optional(pool)
    .await?;

    Ok(row
        .map(|(id, is_open)| OpenInfo { id, is_open })
        .unwrap_or(OpenInfo {
            id: open_id,
            is_open: false,
        }))
}

fn completed_episodes_with_chain_aliases(
    completed: &HashSet<i32>,
    aliases: impl IntoIterator<Item = (i32, i32)>,
) -> HashSet<i32> {
    let mut result = completed.clone();
    for (episode_id, alias_id) in aliases {
        if completed.contains(&episode_id) && alias_id > 0 {
            result.insert(alias_id);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::completed_episodes_with_chain_aliases;
    use std::collections::HashSet;

    #[test]
    fn only_cleared_episodes_grant_their_chain_alias() {
        let cleared = HashSet::from([10503]);
        let result = completed_episodes_with_chain_aliases(
            &cleared,
            [(10503, 503), (10504, 504), (10503, 0)],
        );
        assert!(result.contains(&10503));
        assert!(result.contains(&503));
        assert!(!result.contains(&504));
        assert!(!result.contains(&0));
    }
}
