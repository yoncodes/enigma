use crate::error::AppError;
use database::db::game::explore;
use sonettobuf::{GetExploreSimpleInfoReply, WeekwalkVer2GetInfoReply};
use sqlx::SqlitePool;

mod entry;
mod interaction;
mod items;
mod movement;
mod snapshot;
mod weekwalk_v2;

pub use interaction::{InteractAction, InteractOutcome};
pub use items::UseItemOutcome;
pub use movement::{MoveOutcome, RoleMove};

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

    pub async fn change_weekwalk_v2_group(
        self,
        db: &SqlitePool,
        layer_id: i32,
        battle_id: i32,
        select: i32,
    ) -> Result<sonettobuf::WeekwalkVer2ChangeHeroGroupSelectReply, AppError> {
        weekwalk_v2::change_group(db, self.player_id, layer_id, battle_id, select).await
    }

    pub async fn choose_weekwalk_v2_skill(
        self,
        db: &SqlitePool,
        no: i32,
        skill_ids: Vec<i32>,
    ) -> Result<sonettobuf::WeekwalkVer2ChooseSkillReply, AppError> {
        weekwalk_v2::choose_skill(db, self.player_id, no, skill_ids).await
    }

    pub async fn move_to(
        self,
        db: &SqlitePool,
        posx: i32,
        posy: i32,
        interact_id: Option<i32>,
    ) -> Result<MoveOutcome, AppError> {
        movement::move_to(db, self.player_id, posx, posy, interact_id).await
    }

    pub async fn use_item(
        self,
        db: &SqlitePool,
        uid: i64,
        posx: i32,
        posy: i32,
        interact_id: Option<i32>,
    ) -> Result<UseItemOutcome, AppError> {
        items::use_item(db, self.player_id, uid, posx, posy, interact_id).await
    }

    pub async fn interact(
        self,
        db: &SqlitePool,
        id: i32,
        params: Option<String>,
    ) -> Result<InteractOutcome, AppError> {
        interaction::interact(db, self.player_id, id, params).await
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
