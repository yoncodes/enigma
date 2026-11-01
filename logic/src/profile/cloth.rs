use super::ProfileManager;
use crate::error::AppError;
use database::db::game::cloths;
use sonettobuf::{GetClothInfoReply, PlayerClothInfo};
use sqlx::SqlitePool;

impl ProfileManager {
    pub async fn cloth_info(&self, db: &SqlitePool) -> Result<GetClothInfoReply, AppError> {
        // Also repair accounts that passed the story before the unlock was implemented.
        cloths::reconcile_atonal_sequence(db, self.player_id).await?;
        Ok(GetClothInfoReply {
            cloth_infos: Some(PlayerClothInfo {
                clothes: cloths::get_all(db, self.player_id).await?,
            }),
        })
    }
}
