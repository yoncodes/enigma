use database::db::game::rouge;
use sonettobuf::Rouge2SetSystemIdReply;
use sqlx::SqlitePool;

use crate::error::AppError;

use super::RougeManager;

const ACTIVE_STATE: i32 = 2;
const UNSELECTED_SYSTEM_ID: i32 = 0;

impl RougeManager {
    pub async fn set_system_id(
        self,
        db: &SqlitePool,
        system_id: i32,
    ) -> Result<Rouge2SetSystemIdReply, AppError> {
        if system_id != UNSELECTED_SYSTEM_ID
            && config::configs::get()
                .rouge2_system
                .get(system_id)
                .is_none()
        {
            return Err(AppError::InvalidRequest);
        }
        let system_id = rouge::set_system_id(db, self.player_id, system_id, ACTIVE_STATE)
            .await?
            .ok_or(AppError::InvalidRequest)?;
        Ok(Rouge2SetSystemIdReply {
            system_id: Some(system_id),
        })
    }
}
