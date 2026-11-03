use super::{ExplorationManager, snapshot};
use crate::error::AppError;
use database::db::game::{dungeons, explore::state, open_infos};
use sonettobuf::{ChangeMapReply, GetExploreInfoReply};
use sqlx::SqlitePool;

// OpenEnum.Explore in the client.
const OPEN_ID: i32 = 47;

impl ExplorationManager {
    pub(super) async fn available_maps(self, db: &SqlitePool) -> Result<Vec<i32>, AppError> {
        let mut maps = Vec::new();
        if !open_infos::get_open_info(db, self.player_id, OPEN_ID)
            .await?
            .is_open
        {
            return Ok(maps);
        }
        for scene in config::get().explore_scene.iter() {
            if dungeons::can_start_episode(db, self.player_id, scene.chapter_id, scene.episode_id)
                .await?
            {
                maps.push(scene.id);
            }
        }
        maps.sort_unstable();
        Ok(maps)
    }

    async fn check_map(self, db: &SqlitePool, map_id: i32) -> Result<(), AppError> {
        let scene = config::get()
            .explore_scene
            .get(map_id)
            .ok_or(AppError::InvalidRequest)?;
        if !open_infos::get_open_info(db, self.player_id, OPEN_ID)
            .await?
            .is_open
            || !dungeons::can_start_episode(db, self.player_id, scene.chapter_id, scene.episode_id)
                .await?
        {
            return Err(AppError::InvalidRequest);
        }
        Ok(())
    }

    pub async fn change_map(
        self,
        db: &SqlitePool,
        map_id: i32,
    ) -> Result<ChangeMapReply, AppError> {
        self.check_map(db, map_id).await?;
        let mut tx = db.begin().await?;
        let saved = state::load(&mut tx, self.player_id, map_id).await?;
        let (map, first) = match saved {
            Some(map) => (map, false),
            None => {
                let map = snapshot::initial(
                    config::get()
                        .explore_map
                        .get(map_id)
                        .ok_or(AppError::InvalidRequest)?,
                )?;
                let first = state::insert(&mut tx, self.player_id, &map).await?;
                let map = state::load(&mut tx, self.player_id, map_id)
                    .await?
                    .ok_or_else(|| AppError::Custom("exploration map was not saved".into()))?;
                (map, first)
            }
        };
        state::select(&mut tx, self.player_id, map_id).await?;
        tx.commit().await?;
        Ok(ChangeMapReply {
            map_id: Some(map_id),
            explore_info: Some(snapshot::info(map, first)),
        })
    }

    pub async fn info(self, db: &SqlitePool) -> Result<GetExploreInfoReply, AppError> {
        let mut tx = db.begin().await?;
        let map_id = state::last_map(&mut tx, self.player_id)
            .await?
            .filter(|&id| id != 0)
            .ok_or(AppError::InvalidRequest)?;
        let map = state::load(&mut tx, self.player_id, map_id)
            .await?
            .ok_or(AppError::InvalidRequest)?;
        tx.commit().await?;
        self.check_map(db, map_id).await?;
        Ok(GetExploreInfoReply {
            explore_info: Some(snapshot::info(map, false)),
        })
    }
}
