use crate::{error::AppError, reward, task::UserTask, types::red_dot_id::RedDotId};
use database::{
    db::game::{
        block_packages, buildings, character_interactions, dungeons, player_infos, red_dots,
        room_ob, room_plan, tasks as task_db,
    },
    models::game::heros::UserHeroModel,
};
use sonettobuf::{
    AllotCritterReply, AllotVehicleReply, BlockPermanentInfo, CopyOtherRoomPlanReply,
    DeleteRoadReply, GainProductionLineReply, GainRoomHeroFaithReply, GenerateRoadReply,
    GetBlockPackageInfoReply, GetBlockPermanentInfoReply, GetBuildingInfoReply,
    GetCharacterInteractionBonusReply, GetCharacterInteractionInfoReply, GetOtherRoomObInfoReply,
    GetRoomInfoReply, GetRoomLogReply, GetRoomObInfoReply, GetRoomPlanDetailsReply,
    GetRoomPlanInfoReply, GetRoomShareReply, GetRoomThemeCollectionBonusReply,
    HideBlockPackageReddotReply, HideBuildingReddotReply, ProductionLineAccelerateReply,
    ProductionLineInfoReply, ProductionLineLvUpReply, ReadRoomLogNewReply, ReadRoomSkinReply,
    ResetRoomReply, RoadInfo, RoomConfirmReply, RoomHeroData, RoomLevelUpReply, RoomPlanInfo,
    RoomRevertReply, SetBlockColorReply, SetRoomPlanCoverReply, SetRoomPlanNameReply,
    SetRoomPlanReply, SetRoomSkinReply, SetWaterTypeReply, ShareRoomPlanReply,
    StartCharacterInteractionReply, StartProductionLineReply, SwitchRoomPlanReply, UnUseBlockReply,
    UnUseBuildingReply, UpdateRoomHeroDataReply, UseBlockReply, UseBuildingReply, UseRoomPlanReply,
    UseRoomShareReply,
};
use sqlx::SqlitePool;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug)]
pub struct RoomManager {
    player_id: i64,
}

impl RoomManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }
}

pub struct ProductionStart {
    pub reply: StartProductionLineReply,
    pub consumed_item_ids: Vec<u32>,
    pub consumed_currency_ids: Vec<(i32, i32)>,
    pub material_changes: Vec<(u32, u32, i32)>,
}

pub struct ProductionGain {
    pub reply: GainProductionLineReply,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
}

pub struct RoomReward<T> {
    pub reply: T,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
}

pub struct RoomFaithGain {
    pub reply: GainRoomHeroFaithReply,
    pub changed_hero_ids: Vec<i32>,
    pub material_changes: Vec<(u32, u32, i32)>,
}

pub struct RoomCostUpdate<T> {
    pub reply: T,
    pub consumed_item_ids: Vec<u32>,
    pub consumed_currency_ids: Vec<(i32, i32)>,
    pub material_changes: Vec<(u32, u32, i32)>,
}
mod blocks;
mod hero;
mod info;
mod interaction;
mod layout;
mod manufacture;
mod plan;
mod production;
mod trade;

#[cfg(test)]
use blocks::permanent_infos_for_blocks;
#[cfg(test)]
use plan::room_plan_building_degree;
#[cfg(test)]
use production::{scale_production_rewards, scaled_formula_materials};

#[cfg(test)]
mod tests;
