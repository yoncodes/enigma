use super::PreferenceManager;
use crate::error::AppError;
use common::time::ServerTime;
use database::db::game::bgm;
use sonettobuf::{BgmInfo, GetBgmInfoReply, ReadBgmReply, SetFavoriteBgmReply, SetUseBgmReply};
use sqlx::SqlitePool;

fn has_global_audio_bank(bgm_id: i32) -> bool {
    if bgm_id == 0 {
        return true;
    }

    let configs = config::configs::get();
    configs
        .bgm_switch
        .get(bgm_id)
        .and_then(|bgm| configs.ui_audio.get(bgm.audio))
        .is_some_and(|audio| !audio.bank_name.ends_with("nfc"))
}

impl PreferenceManager {
    pub async fn bgm_info(&self, db: &SqlitePool) -> Result<GetBgmInfoReply, AppError> {
        let (mut bgm_infos, use_bgm_id) = bgm::load_user_bgm(db, self.player_id).await?;
        bgm_infos.retain(|info| info.bgm_id.is_some_and(has_global_audio_bank));

        Ok(GetBgmInfoReply {
            bgm_infos,
            use_bgm_id: use_bgm_id.filter(|id| has_global_audio_bank(*id)),
        })
    }

    pub async fn set_use_bgm(
        &self,
        db: &SqlitePool,
        bgm_id: i32,
    ) -> Result<SetUseBgmReply, AppError> {
        if !has_global_audio_bank(bgm_id) {
            return Err(AppError::InvalidRequest);
        }

        bgm::set_active_bgm(db, self.player_id, bgm_id).await?;
        Ok(SetUseBgmReply {
            bgm_id: Some(bgm_id),
        })
    }

    pub async fn read_bgm(&self, db: &SqlitePool, bgm_id: i32) -> Result<ReadBgmReply, AppError> {
        bgm::mark_bgm_read(db, self.player_id, bgm_id).await?;
        Ok(ReadBgmReply {
            bgm_id: Some(bgm_id),
        })
    }

    pub async fn set_favorite_bgm(
        &self,
        db: &SqlitePool,
        bgm_id: i32,
        favorite: bool,
    ) -> Result<SetFavoriteBgmReply, AppError> {
        bgm::set_bgm_favorite(db, self.player_id, bgm_id, favorite).await?;
        Ok(SetFavoriteBgmReply {
            bgm_id: Some(bgm_id),
            favorite: Some(favorite),
        })
    }

    pub async fn unlock_all_bgms(&self, db: &SqlitePool) -> Result<Vec<BgmInfo>, AppError> {
        let bgm_ids = config::configs::get()
            .bgm_switch
            .iter()
            .filter(|bgm| has_global_audio_bank(bgm.id))
            .map(|bgm| bgm.id)
            .collect::<Vec<_>>();

        Ok(bgm::unlock_bgms(db, self.player_id, &bgm_ids, ServerTime::now_sec_i32()).await?)
    }
}

#[cfg(test)]
mod tests;
