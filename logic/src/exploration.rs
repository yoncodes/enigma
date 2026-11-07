use crate::error::AppError;
use database::db::game::explore;
use sonettobuf::{GetExploreSimpleInfoReply, WeekwalkVer2GetInfoReply};
use sqlx::SqlitePool;

mod entry;
mod snapshot;
mod weekwalk_v2;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug)]
pub struct ExplorationManager {
    player_id: i64,
}

impl ExplorationManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }

    pub async fn simple_info(self, db: &SqlitePool) -> Result<GetExploreSimpleInfoReply, AppError> {
        explore_simple_info(db, self.player_id).await
    }

    pub async fn weekwalk_v2_info(
        self,
        db: &SqlitePool,
    ) -> Result<WeekwalkVer2GetInfoReply, AppError> {
        weekwalk_v2::info(db, self.player_id).await
    }

    pub async fn mark_weekwalk_v2_finished_shown(
        self,
        db: &SqlitePool,
        layer_id: i32,
    ) -> Result<sonettobuf::WeekwalkVer2MarkShowFinishedReply, AppError> {
        weekwalk_v2::mark_finished_shown(db, self.player_id, layer_id).await
    }
}

async fn explore_simple_info(
    db: &SqlitePool,
    player_id: i64,
) -> Result<GetExploreSimpleInfoReply, AppError> {
    let (info, chapters, maps, _) = explore::get_explore_info(db, player_id).await?;
    let unlock_map_ids = ExplorationManager::new(player_id)
        .available_maps(db)
        .await?;

    Ok(GetExploreSimpleInfoReply {
        last_map_id: Some(info.last_map_id),
        chapter_simple: chapters.into_iter().map(Into::into).collect(),
        map_simple: maps.into_iter().map(Into::into).collect(),
        unlock_map_ids,
        is_show_bag: Some(info.is_show_bag),
    })
}
