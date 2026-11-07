use crate::error::AppError;
use database::db::game::milestones;
use sonettobuf::{GetMilestoneInfoReply, MilestoneInfo};
use sqlx::SqlitePool;

#[cfg(test)]
mod tests;

const RACING_TYPE: i32 = 2;

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
}
