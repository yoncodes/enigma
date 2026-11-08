use crate::{activity::activity243_total_stars, error::AppError, reward};
use database::db::game::milestones;
use sonettobuf::{GetMilestoneBonusReply, GetMilestoneInfoReply, MilestoneInfo};
use sqlx::SqlitePool;

#[cfg(test)]
mod tests;

const RACING_TYPE: i32 = 2;

pub type MilestoneClaim = reward::RewardedReply<GetMilestoneBonusReply>;

#[derive(Clone, Copy, Debug)]
pub struct MilestoneManager {
    player_id: i64,
}

impl MilestoneManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }

    pub async fn info(
        self,
        db: &SqlitePool,
        milestone_ids: Vec<i32>,
    ) -> Result<GetMilestoneInfoReply, AppError> {
        if milestone_ids.is_empty() {
            return Err(AppError::InvalidRequest);
        }
        let tables = config::get();
        let mut infos = Vec::with_capacity(milestone_ids.len());
        for milestone_id in milestone_ids {
            let milestone = tables
                .milestone
                .iter()
                .find(|row| row.milestone_id == milestone_id)
                .filter(|row| row.r#type == RACING_TYPE)
                .ok_or(AppError::InvalidRequest)?;
            if !tables
                .activity243_episode
                .iter()
                .any(|episode| episode.activity_id == milestone.activity_id)
            {
                return Err(AppError::InvalidRequest);
            }
            infos.push(MilestoneInfo {
                milestone_id: Some(milestone_id),
                get_bonus_id: Some(
                    milestones::claimed_bonus_id(db, self.player_id, milestone_id).await?,
                ),
                progress: Some(0),
            });
        }
        Ok(GetMilestoneInfoReply {
            milestone_infos: infos,
        })
    }

    pub async fn claim(
        self,
        db: &SqlitePool,
        milestone_id: i32,
    ) -> Result<MilestoneClaim, AppError> {
        let tables = config::get();
        let milestone = tables
            .milestone
            .iter()
            .find(|row| row.milestone_id == milestone_id)
            .filter(|row| row.r#type == RACING_TYPE)
            .ok_or(AppError::InvalidRequest)?;
        let progress = activity243_total_stars(db, self.player_id, milestone.activity_id).await?;
        let claimed = milestones::claimed_bonus_id(db, self.player_id, milestone_id).await?;
        let mut tiers = tables
            .milestone_bonus
            .iter()
            .filter(|tier| {
                tier.milestone_id == milestone_id
                    && tier.bonus_id > claimed
                    && tier.need_progress <= progress
            })
            .collect::<Vec<_>>();
        tiers.sort_by_key(|tier| tier.bonus_id);
        let next = tiers
            .last()
            .map(|tier| tier.bonus_id)
            .ok_or(AppError::InvalidRequest)?;
        let mut reward_set = reward::RewardSet::default();
        for tier in tiers {
            reward_set.extend(reward::parse_strict(&tier.bonus)?);
        }
        let material_changes = reward_set.material_changes();

        let mut tx = db.begin().await?;
        if !milestones::advance_claim(&mut tx, self.player_id, milestone_id, claimed, next).await? {
            return Err(AppError::InvalidRequest);
        }
        let rewards = reward::RewardManager::new(self.player_id)
            .apply_in_transaction(&mut tx, db, reward_set)
            .await?;
        tx.commit().await?;

        Ok(MilestoneClaim {
            reply: GetMilestoneBonusReply {
                milestone_id: Some(milestone_id),
                get_bonus_id: Some(next),
            },
            rewards,
            material_changes,
        })
    }
}
