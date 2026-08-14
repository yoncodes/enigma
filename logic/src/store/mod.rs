mod catalog;
mod charge;
mod purchase;
mod time;

use crate::error::AppError;
pub use charge::NewOrderResult;
pub use purchase::BuyGoodsResult;
use sonettobuf::{ChargeInfo, GetStoreInfosReply, SelectionInfo};
use sqlx::SqlitePool;

#[cfg(test)]
use charge::{
    battle_pass_pay_status, battle_pass_purchase_bonus, charge_goods_attachment,
    charge_goods_diamond_bonus,
};
#[cfg(test)]
use purchase::purchase_cost;
pub(crate) use time::*;

#[derive(Clone, Copy, Debug)]
pub struct StoreManager {
    player_id: i64,
}

impl StoreManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }

    pub async fn infos(
        &self,
        db: &SqlitePool,
        store_ids: &[i32],
    ) -> Result<GetStoreInfosReply, AppError> {
        catalog::store_infos(db, self.player_id, store_ids).await
    }

    pub async fn charge_infos(&self, db: &SqlitePool) -> Result<Vec<ChargeInfo>, AppError> {
        catalog::charge_infos(db, self.player_id).await
    }

    pub async fn buy_goods(
        &self,
        db: &SqlitePool,
        store_id: i32,
        goods_id: i32,
        num: i32,
        select_cost: Option<i32>,
    ) -> Result<BuyGoodsResult, AppError> {
        purchase::buy_goods(db, self.player_id, store_id, goods_id, num, select_cost).await
    }

    pub async fn new_order(
        &self,
        db: &SqlitePool,
        goods_id: i32,
        currency: Option<String>,
        selections: &[SelectionInfo],
        now: i64,
    ) -> Result<NewOrderResult, AppError> {
        charge::new_order(db, self.player_id, goods_id, currency, selections, now).await
    }
}

#[cfg(test)]
mod test;
