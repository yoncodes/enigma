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
mod tests {
    use super::*;
    use config::configs;
    use sqlx::SqlitePool;

    async fn test_pool(player_id: i64) -> SqlitePool {
        let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("data/excel2json");
        let _ = config::init(data_dir.to_str().unwrap());
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (?, ?, 0, 0)",
        )
        .bind(player_id)
        .bind(format!("teaching-{player_id}"))
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    async fn complete_episode(pool: &SqlitePool, player_id: i64, episode_id: i32, star: i32) {
        let episode = configs::get().episode.get(episode_id).unwrap();
        sqlx::query(
            "INSERT INTO user_dungeons
             (user_id, chapter_id, episode_id, star, challenge_count, has_record,
              left_return_all_num, today_pass_num, today_total_num, created_at, updated_at)
             VALUES (?, ?, ?, ?, 0, 0, 1, 0, 0, 0, 0)",
        )
        .bind(player_id)
        .bind(episode.chapter_id)
        .bind(episode_id)
        .bind(star)
        .execute(pool)
        .await
        .unwrap();
    }

    #[test]
    fn generated_teaching_tables_preserve_teaching_and_pre_episode_groups() {
        let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("data/excel2json");
        let _ = config::init(data_dir.to_str().unwrap());
        let tables = configs::get();

        assert_eq!(tables.teaching.len(), 4);
        assert_eq!(tables.teaching_episode.len(), 8);
        for teaching in tables.teaching.iter() {
            let episodes = tables
                .teaching_episode
                .iter()
                .filter(|episode| episode.teaching == teaching.id)
                .collect::<Vec<_>>();
            assert_eq!(episodes.len(), 2);
            let first = episodes
                .iter()
                .find(|episode| episode.pre_episode == 0)
                .unwrap();
            assert!(
                episodes
                    .iter()
                    .any(|episode| { episode.pre_episode == first.id && episode.id != first.id })
            );
        }
        assert!(
            tables
                .teaching_episode
                .iter()
                .all(|episode| tables.teaching.get(episode.teaching).is_some())
        );
    }

    #[tokio::test]
    async fn snapshot_empty_state_has_no_teaching_progress() {
        let player_id = 901;
        let pool = test_pool(player_id).await;
        let info = snapshot(&pool, player_id).await.unwrap();

        assert!(info.pass_episodes.is_empty());
        assert!(info.teachinges.is_empty());
    }

    #[tokio::test]
    async fn snapshot_ignores_zero_star_dungeon_rows() {
        let player_id = 907;
        let pool = test_pool(player_id).await;
        let episode_id = configs::get().teaching_episode.iter().next().unwrap().id;
        complete_episode(&pool, player_id, episode_id, 0).await;

        let info = snapshot(&pool, player_id).await.unwrap();
        assert!(info.pass_episodes.is_empty());
        assert!(info.teachinges.is_empty());
    }

    #[tokio::test]
    async fn snapshot_one_completed_episode_is_passed_but_not_claimable() {
        let player_id = 902;
        let pool = test_pool(player_id).await;
        let teaching = configs::get().teaching.iter().next().unwrap();
        let episodes = configs::get()
            .teaching_episode
            .iter()
            .filter(|episode| episode.teaching == teaching.id)
            .collect::<Vec<_>>();
        complete_episode(&pool, player_id, episodes[0].id, 1).await;

        let info = snapshot(&pool, player_id).await.unwrap();
        assert!(info.pass_episodes.contains(&episodes[0].id));
        assert!(!info.pass_episodes.contains(&episodes[1].id));
        assert!(
            info.teachinges
                .iter()
                .all(|entry| entry.teaching_id != Some(teaching.id))
        );
    }

    #[tokio::test]
    async fn snapshot_completing_all_configured_episodes_emits_status_one() {
        let player_id = 903;
        let pool = test_pool(player_id).await;
        let teaching = configs::get().teaching.iter().next().unwrap();
        let episodes = configs::get()
            .teaching_episode
            .iter()
            .filter(|episode| episode.teaching == teaching.id)
            .collect::<Vec<_>>();
        for episode in &episodes {
            complete_episode(&pool, player_id, episode.id, 1).await;
        }

        let info = snapshot(&pool, player_id).await.unwrap();
        assert!(
            episodes
                .iter()
                .all(|episode| info.pass_episodes.contains(&episode.id))
        );
        assert_eq!(
            info.teachinges
                .iter()
                .find(|entry| entry.teaching_id == Some(teaching.id))
                .unwrap()
                .status,
            Some(1)
        );
    }

    #[tokio::test]
    async fn snapshot_completed_and_claimed_emits_status_two() {
        let player_id = 905;
        let pool = test_pool(player_id).await;
        let teaching = configs::get().teaching.iter().next().unwrap();
        let episodes = configs::get()
            .teaching_episode
            .iter()
            .filter(|episode| episode.teaching == teaching.id)
            .collect::<Vec<_>>();
        for episode in &episodes {
            complete_episode(&pool, player_id, episode.id, 1).await;
        }
        sqlx::query(
            "INSERT INTO user_teaching_bonus_claims (user_id, teaching_id)
             VALUES (?, ?)",
        )
        .bind(player_id)
        .bind(teaching.id)
        .execute(&pool)
        .await
        .unwrap();

        let info = snapshot(&pool, player_id).await.unwrap();

        assert_eq!(
            info.teachinges
                .iter()
                .find(|entry| entry.teaching_id == Some(teaching.id))
                .unwrap()
                .status,
            Some(2)
        );
    }

    #[tokio::test]
    async fn claim_bonus_inserts_marker_and_applies_config_reward_once() {
        let player_id = 906;
        let pool = test_pool(player_id).await;
        let teaching = configs::get().teaching.iter().next().unwrap();
        let episodes = configs::get()
            .teaching_episode
            .iter()
            .filter(|episode| episode.teaching == teaching.id)
            .collect::<Vec<_>>();
        for episode in &episodes {
            complete_episode(&pool, player_id, episode.id, 1).await;
        }

        let claim = claim_bonus(&pool, player_id, teaching.id).await.unwrap();
        assert_eq!(claim.teaching_id, teaching.id);
        assert_eq!(claim.rewards.currency_ids, vec![(21, 30)]);
        assert_eq!(claim.material_changes, vec![(2, 21, 30)]);
        assert_eq!(
            sqlx::query_scalar::<_, i32>(
                "SELECT quantity FROM currencies
                 WHERE user_id = ? AND currency_id = 21",
            )
            .bind(player_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            30
        );

        assert!(matches!(
            claim_bonus(&pool, player_id, teaching.id).await,
            Err(AppError::InvalidRequest)
        ));
        assert_eq!(
            sqlx::query_scalar::<_, i32>(
                "SELECT quantity FROM currencies
                 WHERE user_id = ? AND currency_id = 21",
            )
            .bind(player_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            30
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM user_teaching_bonus_claims
                 WHERE user_id = ? AND teaching_id = ?",
            )
            .bind(player_id)
            .bind(teaching.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn claim_bonus_rejects_unknown_or_incomplete_without_mutation() {
        let player_id = 908;
        let pool = test_pool(player_id).await;
        let teaching = configs::get().teaching.iter().next().unwrap();
        let unknown_id = configs::get()
            .teaching
            .iter()
            .map(|row| row.id)
            .max()
            .unwrap()
            + 1;

        assert!(matches!(
            claim_bonus(&pool, player_id, unknown_id).await,
            Err(AppError::InvalidRequest)
        ));
        assert!(matches!(
            claim_bonus(&pool, player_id, teaching.id).await,
            Err(AppError::InvalidRequest)
        ));
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM user_teaching_bonus_claims WHERE user_id = ?",
            )
            .bind(player_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
        assert!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM currencies WHERE user_id = ?",)
                .bind(player_id)
                .fetch_one(&pool)
                .await
                .unwrap()
                == 0
        );
    }

    #[tokio::test]
    async fn snapshot_retains_multiple_teachings_and_is_idempotent() {
        let player_id = 904;
        let pool = test_pool(player_id).await;
        let teachings = configs::get().teaching.iter().take(2).collect::<Vec<_>>();
        let first_episodes = configs::get()
            .teaching_episode
            .iter()
            .filter(|episode| episode.teaching == teachings[0].id)
            .collect::<Vec<_>>();
        for episode in first_episodes {
            complete_episode(&pool, player_id, episode.id, 1).await;
        }
        let second_episode = configs::get()
            .teaching_episode
            .iter()
            .find(|episode| episode.teaching == teachings[1].id)
            .unwrap();
        complete_episode(&pool, player_id, second_episode.id, 1).await;

        let first = snapshot(&pool, player_id).await.unwrap();
        let second = snapshot(&pool, player_id).await.unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first
                .teachinges
                .iter()
                .find(|entry| entry.teaching_id == Some(teachings[0].id))
                .unwrap()
                .status,
            Some(1)
        );
        assert!(
            first
                .teachinges
                .iter()
                .all(|entry| entry.teaching_id != Some(teachings[1].id))
        );
    }
}
