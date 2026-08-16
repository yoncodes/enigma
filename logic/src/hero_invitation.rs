use crate::{error::AppError, reward, task::UserTask};
use config::GameDB;
use database::db::game::{dungeons, hero_invitation, tasks};
use sonettobuf::{
    GainFinalInviteRewardReply, GainInviteRewardReply, GetHeroInvitationInfoReply,
    HeroInvitationInfo,
};
use sqlx::SqlitePool;
use std::collections::HashSet;

const FINAL_REWARD_CONST_ID: i32 = 1902;
const FINAL_CLAIM_ID: i32 = 0;

#[derive(Clone, Copy, Debug)]
pub struct HeroInvitationManager {
    player_id: i64,
}

pub struct HeroInvitationClaim<T> {
    pub reply: T,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
    pub newly_claimed: bool,
    pub updated_tasks: Vec<UserTask>,
}

impl HeroInvitationManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }

    pub async fn info(
        &self,
        db: &SqlitePool,
        tables: &GameDB,
    ) -> Result<GetHeroInvitationInfoReply, AppError> {
        Ok(GetHeroInvitationInfoReply {
            info: Some(
                self.snapshot(db, tables, common::time::ServerTime::now_sec_i32())
                    .await?,
            ),
        })
    }

    pub async fn gain_reward(
        &self,
        db: &SqlitePool,
        tables: &GameDB,
        invite_id: i32,
    ) -> Result<HeroInvitationClaim<GainInviteRewardReply>, AppError> {
        let invitation = tables
            .hero_invitation
            .get(invite_id)
            .ok_or(AppError::InvalidRequest)?;
        let claims = hero_invitation::get_claims(db, self.player_id).await?;
        let already_claimed = claims.contains(&invite_id);

        let reward_set = if already_claimed {
            reward::RewardSet::default()
        } else {
            let finished_elements = dungeons::get_finished_elements(db, self.player_id).await?;
            if !finished_elements.contains(&invitation.element_id) {
                return Err(AppError::InvalidRequest);
            }
            reward::parse_strict(&invitation.reward_display_list)?
        };
        let material_changes = reward_set.material_changes();

        let mut tx = db.begin().await?;
        let newly_claimed =
            hero_invitation::claim_in_transaction(&mut tx, self.player_id, invite_id).await?;
        let rewards = if newly_claimed {
            reward::RewardManager::new(self.player_id)
                .apply_in_transaction(&mut tx, db, reward_set)
                .await?
        } else {
            reward::AppliedRewards::default()
        };
        let updated_tasks =
            tasks::sync_hero_invitation_claims_in_transaction(&mut tx, self.player_id).await?;
        tx.commit().await?;

        Ok(HeroInvitationClaim {
            reply: GainInviteRewardReply {
                info: Some(
                    self.snapshot(db, tables, common::time::ServerTime::now_sec_i32())
                        .await?,
                ),
            },
            rewards,
            material_changes: if newly_claimed {
                material_changes
            } else {
                Vec::new()
            },
            newly_claimed,
            updated_tasks,
        })
    }

    pub async fn gain_final_reward(
        &self,
        db: &SqlitePool,
        tables: &GameDB,
    ) -> Result<HeroInvitationClaim<GainFinalInviteRewardReply>, AppError> {
        let claims = hero_invitation::get_claims(db, self.player_id).await?;
        let already_claimed = claims.contains(&FINAL_CLAIM_ID);
        let reward_set = if already_claimed {
            reward::RewardSet::default()
        } else {
            let invite_ids = tables
                .hero_invitation
                .iter()
                .map(|row| row.id)
                .collect::<Vec<_>>();
            if !invite_ids.iter().all(|id| claims.contains(id)) {
                return Err(AppError::InvalidRequest);
            }
            let reward = tables
                .r#const
                .get(FINAL_REWARD_CONST_ID)
                .ok_or(AppError::InvalidRequest)?;
            reward::parse_strict(&reward.value)?
        };
        let material_changes = reward_set.material_changes();

        let mut tx = db.begin().await?;
        let newly_claimed =
            hero_invitation::claim_in_transaction(&mut tx, self.player_id, FINAL_CLAIM_ID).await?;
        let rewards = if newly_claimed {
            reward::RewardManager::new(self.player_id)
                .apply_in_transaction(&mut tx, db, reward_set)
                .await?
        } else {
            reward::AppliedRewards::default()
        };
        tx.commit().await?;

        Ok(HeroInvitationClaim {
            reply: GainFinalInviteRewardReply {
                info: Some(
                    self.snapshot(db, tables, common::time::ServerTime::now_sec_i32())
                        .await?,
                ),
            },
            rewards,
            material_changes: if newly_claimed {
                material_changes
            } else {
                Vec::new()
            },
            newly_claimed,
            updated_tasks: Vec::new(),
        })
    }

    async fn snapshot(
        &self,
        db: &SqlitePool,
        tables: &GameDB,
        now_sec: i32,
    ) -> Result<HeroInvitationInfo, AppError> {
        let claims = hero_invitation::get_claims(db, self.player_id).await?;
        let final_reward = claims.contains(&FINAL_CLAIM_ID);
        let finished_elements = dungeons::get_finished_elements(db, self.player_id)
            .await?
            .into_iter()
            .collect::<HashSet<_>>();

        let mut invitations = tables.hero_invitation.iter().collect::<Vec<_>>();
        invitations.sort_unstable_by_key(|row| row.id);
        let mut opened_invite = Vec::new();
        for invitation in invitations {
            let element = tables
                .chapter_map_element
                .get(invitation.element_id)
                .ok_or(AppError::InvalidRequest)?;
            let time_open = invitation.open_time.is_empty()
                || common::time::ServerTime::config_datetime_sec(&invitation.open_time)
                    .is_some_and(|open_time| now_sec >= open_time);
            if time_open && map_element_condition_met(&element.condition, &finished_elements) {
                opened_invite.push(invitation.id);
            }
        }

        Ok(HeroInvitationInfo {
            opened_invite,
            gain_reward: claims
                .into_iter()
                .filter(|invite_id| *invite_id > FINAL_CLAIM_ID)
                .collect(),
            final_reward: Some(final_reward),
        })
    }
}

fn map_element_condition_met(condition: &str, finished_elements: &HashSet<i32>) -> bool {
    let condition = condition.trim();
    if condition.is_empty() {
        return true;
    }

    condition
        .strip_prefix("ChapterMapElement=")
        .and_then(|id| id.parse::<i32>().ok())
        .is_some_and(|id| finished_elements.contains(&id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

    async fn test_pool(player_id: i64) -> SqlitePool {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (?, ?, 0, 0)",
        )
        .bind(player_id)
        .bind(format!("hero-invitation-{player_id}"))
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    async fn finish_element(pool: &SqlitePool, player_id: i64, element_id: i32) {
        sqlx::query(
            "INSERT INTO user_dungeon_elements
             (user_id, element_id, is_finished, puzzle_progress, puzzle_updated_at)
             VALUES (?, ?, 1, '', 0)",
        )
        .bind(player_id)
        .bind(element_id)
        .execute(pool)
        .await
        .unwrap();
    }

    #[test]
    fn slash_config_times_are_supported_for_invitation_rows() {
        assert!(common::time::ServerTime::config_datetime_sec("2024/05/19 05:00:00").is_some());
        assert!(!map_element_condition_met(
            "ChapterMapElement=311101",
            &HashSet::new()
        ));
    }

    #[tokio::test]
    async fn projection_follows_finished_element_chain_and_claim_markers() {
        let player_id = 5631;
        let pool = test_pool(player_id).await;
        for invitation in config::configs::get().hero_invitation.iter() {
            finish_element(&pool, player_id, invitation.element_id).await;
            sqlx::query(
                "INSERT INTO user_hero_invitation_claims (user_id, invite_id, claimed_at)
                 VALUES (?, ?, 0)",
            )
            .bind(player_id)
            .bind(invitation.id)
            .execute(&pool)
            .await
            .unwrap();
        }
        sqlx::query(
            "INSERT INTO user_hero_invitation_claims (user_id, invite_id, claimed_at)
             VALUES (?, 0, 0)",
        )
        .bind(player_id)
        .execute(&pool)
        .await
        .unwrap();

        let info = HeroInvitationManager::new(player_id)
            .info(&pool, config::configs::get())
            .await
            .unwrap()
            .info
            .unwrap();
        assert_eq!(info.opened_invite, (1..=14).collect::<Vec<_>>());
        assert_eq!(info.gain_reward, (1..=14).collect::<Vec<_>>());
        assert_eq!(info.final_reward, Some(true));
    }

    #[tokio::test]
    async fn individual_claim_is_idempotent_and_final_claim_is_gated() {
        let player_id = 5632;
        let pool = test_pool(player_id).await;
        finish_element(&pool, player_id, 311101).await;
        let manager = HeroInvitationManager::new(player_id);

        assert!(matches!(
            manager
                .gain_final_reward(&pool, config::configs::get())
                .await,
            Err(AppError::InvalidRequest)
        ));
        let first = manager
            .gain_reward(&pool, config::configs::get(), 1)
            .await
            .unwrap();
        assert!(first.newly_claimed);
        assert_eq!(first.rewards.item_ids, vec![120012]);
        let second = manager
            .gain_reward(&pool, config::configs::get(), 1)
            .await
            .unwrap();
        assert!(!second.newly_claimed);
        assert!(second.rewards.item_ids.is_empty());
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM user_hero_invitation_claims WHERE user_id = ?",
            )
            .bind(player_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn concurrent_individual_claims_grant_the_reward_once() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let url = format!(
            "sqlite:file:hero-invitation-claim-{}?mode=memory&cache=shared",
            common::time::ServerTime::now_ms()
        );
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect(&url)
            .await
            .unwrap();
        database::run_migrations(&pool).await.unwrap();
        let player_id = 5636;
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (?, 'hero-invitation-concurrent', 0, 0)",
        )
        .bind(player_id)
        .execute(&pool)
        .await
        .unwrap();
        finish_element(&pool, player_id, 311101).await;

        let manager = HeroInvitationManager::new(player_id);
        let (left, right) = tokio::join!(
            manager.gain_reward(&pool, config::configs::get(), 1),
            manager.gain_reward(&pool, config::configs::get(), 1)
        );
        let left = left.unwrap();
        let right = right.unwrap();
        assert_eq!(
            usize::from(left.newly_claimed) + usize::from(right.newly_claimed),
            1
        );
        assert_eq!(left.updated_tasks.len() + right.updated_tasks.len(), 3);
        assert_eq!(
            sqlx::query_scalar::<_, i32>(
                "SELECT quantity FROM items WHERE user_id = ? AND item_id = 120012",
            )
            .bind(player_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i32>(
                "SELECT progress FROM user_tasks
                 WHERE user_id = ? AND type_id = 11 AND task_id = 110918",
            )
            .bind(player_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM user_hero_invitation_claims
                 WHERE user_id = ? AND invite_id = 1",
            )
            .bind(player_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn invalid_reward_rolls_back_the_claim_marker() {
        let player_id = 5633;
        let pool = test_pool(player_id).await;
        finish_element(&pool, player_id, 311101).await;
        sqlx::query(
            "CREATE TRIGGER fail_hero_invitation_reward
             BEFORE INSERT ON items
             WHEN NEW.user_id = 5633 AND NEW.item_id = 120012
             BEGIN SELECT RAISE(ABORT, 'test reward failure'); END",
        )
        .execute(&pool)
        .await
        .unwrap();

        assert!(
            HeroInvitationManager::new(player_id)
                .gain_reward(&pool, config::configs::get(), 1)
                .await
                .is_err()
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM user_hero_invitation_claims WHERE user_id = ?",
            )
            .bind(player_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn task_sync_failure_rolls_back_the_claim_and_reward() {
        let player_id = 5637;
        let pool = test_pool(player_id).await;
        finish_element(&pool, player_id, 311101).await;
        sqlx::query(
            "CREATE TRIGGER fail_hero_invitation_task
             BEFORE INSERT ON user_tasks
             WHEN NEW.user_id = 5637 AND NEW.type_id = 11
             BEGIN SELECT RAISE(ABORT, 'test task failure'); END",
        )
        .execute(&pool)
        .await
        .unwrap();

        assert!(
            HeroInvitationManager::new(player_id)
                .gain_reward(&pool, config::configs::get(), 1)
                .await
                .is_err()
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM user_hero_invitation_claims WHERE user_id = ?",
            )
            .bind(player_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM items WHERE user_id = ? AND item_id = 120012",
            )
            .bind(player_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn final_claim_uses_the_configured_const_reward_after_all_invites() {
        let player_id = 5634;
        let pool = test_pool(player_id).await;
        for invitation in config::configs::get().hero_invitation.iter() {
            finish_element(&pool, player_id, invitation.element_id).await;
            sqlx::query(
                "INSERT INTO user_hero_invitation_claims (user_id, invite_id, claimed_at)
                 VALUES (?, ?, 0)",
            )
            .bind(player_id)
            .bind(invitation.id)
            .execute(&pool)
            .await
            .unwrap();
        }

        let claim = HeroInvitationManager::new(player_id)
            .gain_final_reward(&pool, config::configs::get())
            .await
            .unwrap();
        assert!(claim.newly_claimed);
        assert_eq!(claim.rewards.skin_gains[0].skin_id, 302304);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM user_hero_invitation_claims
                 WHERE user_id = ? AND invite_id = 0",
            )
            .bind(player_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
    }
}
