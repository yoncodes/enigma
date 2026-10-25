use crate::error::AppError;
use database::db::game::{player_infos, user_stats};
use sonettobuf::{StatInfoPush, UpdateClientStatBaseInfoReply};
use sqlx::SqlitePool;

#[derive(Clone, Copy, Debug)]
pub struct StatManager {
    player_id: i64,
}

impl StatManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }

    pub async fn base_info(&self, db: &SqlitePool) -> Result<StatInfoPush, AppError> {
        let stats = user_stats::get_user_stats(db, self.player_id)
            .await?
            .ok_or_else(|| AppError::Custom("User stats not found".to_string()))?;

        let is_first_login = stats.is_first_login;
        let mut push: StatInfoPush = stats.into();
        if is_first_login {
            push.player_info = player_infos::get_player_info_data(db, self.player_id)
                .await?
                .map(Into::into);
        }
        user_stats::set_not_first_login(db, self.player_id).await?;
        Ok(push)
    }

    pub fn update_base_info(&self) -> UpdateClientStatBaseInfoReply {
        UpdateClientStatBaseInfoReply::default()
    }
}

#[cfg(test)]
mod tests;
