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
mod tests;
