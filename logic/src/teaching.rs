use crate::error::AppError;
use crate::reward;
use config::configs;
use database::db::game::{dungeons, teaching as teaching_db};
use sonettobuf::{Teaching, TeachingInfo};
use sqlx::SqlitePool;
use std::collections::HashSet;

pub struct BonusClaim {
    pub teaching_id: i32,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
}

pub fn is_teaching_episode(episode_id: i32) -> bool {
    configs::get().teaching_episode.get(episode_id).is_some()
}

pub async fn snapshot(db: &SqlitePool, player_id: i64) -> Result<TeachingInfo, AppError> {
    let completed_episodes = dungeons::get_user_dungeons(db, player_id)
        .await?
        .into_iter()
        .filter(|dungeon| dungeon.star > 0)
        .map(|dungeon| dungeon.episode_id)
        .collect::<HashSet<_>>();
    let claimed_teachings = teaching_db::claimed_teaching_ids(db, player_id).await?;
    let tables = configs::get();

    let teachinges = tables
        .teaching
        .iter()
        .filter_map(|teaching| {
            let episodes = tables
                .teaching_episode
                .iter()
                .filter(|episode| episode.teaching == teaching.id)
                .map(|episode| episode.id)
                .collect::<Vec<_>>();
            (!episodes.is_empty()
                && episodes
                    .iter()
                    .all(|episode_id| completed_episodes.contains(episode_id)))
            .then_some(Teaching {
                teaching_id: Some(teaching.id),
                status: Some(if claimed_teachings.contains(&teaching.id) {
                    2
                } else {
                    1
                }),
            })
        })
        .collect();
    let pass_episodes = tables
        .teaching_episode
        .iter()
        .filter(|episode| completed_episodes.contains(&episode.id))
        .map(|episode| episode.id)
        .collect();

    Ok(TeachingInfo {
        teachinges,
        pass_episodes,
    })
}

pub async fn claim_bonus(
    db: &SqlitePool,
    player_id: i64,
    teaching_id: i32,
) -> Result<BonusClaim, AppError> {
    let tables = configs::get();
    let teaching = tables
        .teaching
        .get(teaching_id)
        .ok_or(AppError::InvalidRequest)?;
    let episodes = tables
        .teaching_episode
        .iter()
        .filter(|episode| episode.teaching == teaching_id)
        .map(|episode| episode.id)
        .collect::<Vec<_>>();
    if episodes.is_empty() {
        return Err(AppError::InvalidRequest);
    }

    let reward_set = reward::parse(&teaching.bonus);
    let material_changes = reward_set.material_changes();
    let mut tx = db.begin().await?;
    for episode_id in episodes {
        if dungeons::episode_star_in_transaction(&mut tx, player_id, episode_id).await? <= 0 {
            return Err(AppError::InvalidRequest);
        }
    }
    if !teaching_db::claim_in_transaction(&mut tx, player_id, teaching_id).await? {
        return Err(AppError::InvalidRequest);
    }

    let rewards = reward::RewardManager::new(player_id)
        .apply_in_transaction(&mut tx, db, reward_set)
        .await?;
    tx.commit().await?;

    Ok(BonusClaim {
        teaching_id,
        rewards,
        material_changes,
    })
}

#[cfg(test)]
mod tests;
