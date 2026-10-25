use super::RoomManager;
use crate::error::AppError;
use sonettobuf::{
    BuyManufactureBuildingInfoReply, GetFrozenItemInfoReply, GetManufactureInfoReply,
    ManuBuildingUpgradeReply, ManufactureAccelerateReply, MaterialData, OperationInfo,
    ReapFinishSlotReply, SelectSlotProductionPlanReply,
};
use sqlx::SqlitePool;

mod building;
mod info;
mod production;

pub struct CostUpdate<T> {
    pub reply: T,
    pub item_ids: Vec<u32>,
    pub currency_ids: Vec<(i32, i32)>,
    pub material_changes: Vec<(u32, u32, i32)>,
}

impl RoomManager {
    pub async fn manufacture_info(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
    ) -> Result<GetManufactureInfoReply, AppError> {
        info::manufacture_info(db, self.player_id, tables).await
    }

    pub async fn frozen_item_info(
        &self,
        db: &SqlitePool,
    ) -> Result<GetFrozenItemInfoReply, AppError> {
        info::frozen_item_info(db, self.player_id).await
    }

    pub async fn buy_manufacture_building(
        &self,
        db: &SqlitePool,
        building_id: i32,
        tables: &config::GameDB,
    ) -> Result<CostUpdate<BuyManufactureBuildingInfoReply>, AppError> {
        building::buy_manufacture_building(db, self.player_id, building_id, tables).await
    }

    pub async fn upgrade_manufacture_building(
        &self,
        db: &SqlitePool,
        building_uid: i64,
        tables: &config::GameDB,
    ) -> Result<CostUpdate<ManuBuildingUpgradeReply>, AppError> {
        building::manu_building_upgrade(db, self.player_id, building_uid, tables).await
    }

    pub async fn select_production_plan(
        &self,
        db: &SqlitePool,
        building_uid: i64,
        operation_infos: &[OperationInfo],
        tables: &config::GameDB,
    ) -> Result<SelectSlotProductionPlanReply, AppError> {
        production::select_slot_production_plan(
            db,
            self.player_id,
            building_uid,
            operation_infos,
            tables,
        )
        .await
    }

    pub async fn accelerate_manufacture(
        &self,
        db: &SqlitePool,
        building_uid: i64,
        slot_id: i32,
        use_item_data: Option<MaterialData>,
        tables: &config::GameDB,
    ) -> Result<CostUpdate<ManufactureAccelerateReply>, AppError> {
        production::manufacture_accelerate(
            db,
            self.player_id,
            building_uid,
            slot_id,
            use_item_data,
            tables,
        )
        .await
    }

    pub async fn reap_finished_slots(
        &self,
        db: &SqlitePool,
        building_uid: i64,
        tables: &config::GameDB,
    ) -> Result<ReapFinishSlotReply, AppError> {
        production::reap_finish_slot(db, self.player_id, building_uid, tables).await
    }
}

#[cfg(test)]
mod tests;
