use crate::{error::AppError, reward, types::red_dot_id::RedDotId};
use common::time::ServerTime;
use database::{
    db::game::{currencies, equipment as equipment_db, items, player_infos, red_dots},
    models::game::{
        heros::{HeroModel, InsightUpgrade, UserHeroModel},
        items::UserItemModel,
    },
};
use sonettobuf::{
    AutoUseExpirePowerItemReply, BuyPowerReply, Currency, CurrencyExchangeNo, EatEquip, Equip,
    EquipBreakReply, EquipDecomposeReply, EquipLockReply, EquipRefineReply, ExchangeDiamondReply,
    ExchangeSameCurrencyReply, GetBuyPowerInfoReply, GetCurrencyListReply, GetEquipInfoReply,
    GetItemListReply, InsightItem, Item, M2qEntry, MarkReadSubType21Reply,
    PopExchangeSameCurrencyReply, PowerItem, UseInsightItemReply, UseItemReply, UsePowerItemInfo,
    UsePowerItemListReply, UsePowerItemReply,
};
use sqlx::SqlitePool;
use std::collections::BTreeMap;

#[repr(i32)]
enum ItemSubType {
    EquipmentLevelUp = 84,
}

#[repr(i32)]
enum EquipmentLevelEffectType {
    All = 1,
    Specify = 2,
}

impl EquipmentLevelEffectType {
    fn from_id(id: i32) -> Option<Self> {
        match id {
            id if id == Self::All as i32 => Some(Self::All),
            id if id == Self::Specify as i32 => Some(Self::Specify),
            _ => None,
        }
    }
}
mod account;
mod currency;
mod equipment;
mod item;
mod item_effect;
mod power;

use currency::*;
#[cfg(test)]
use item::item_rewards;
use item::*;
use item_effect::{target_item_rewards, use_equipment_level_item};
use power::*;

#[derive(Clone, Copy, Debug)]
pub struct InventoryManager {
    player_id: i64,
}

pub struct ItemChanges {
    pub items: Vec<Item>,
    pub power_items: Vec<PowerItem>,
    pub insight_items: Vec<InsightItem>,
}

impl InventoryManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }

    pub async fn currency_list(
        self,
        db: &SqlitePool,
        currency_ids: Vec<i32>,
    ) -> Result<GetCurrencyListReply, AppError> {
        currency_list(db, self.player_id, currency_ids).await
    }

    pub async fn currency_snapshots(
        self,
        db: &SqlitePool,
        currency_ids: impl IntoIterator<Item = i32>,
    ) -> Result<Vec<Currency>, AppError> {
        let mut snapshots = Vec::new();
        for currency_id in currency_ids {
            if let Some(currency) =
                currencies::get_currency(db, self.player_id, currency_id).await?
            {
                snapshots.push(currency.into());
            }
        }
        Ok(snapshots)
    }

    pub async fn item_snapshots(
        self,
        db: &SqlitePool,
        item_ids: Vec<u32>,
        power_item_ids: Vec<i32>,
        insight_item_ids: Vec<i32>,
    ) -> Result<ItemChanges, AppError> {
        let mut changes = ItemChanges {
            items: Vec::new(),
            power_items: Vec::new(),
            insight_items: Vec::new(),
        };
        for item_id in item_ids {
            if let Some(item) = items::get_item(db, self.player_id, item_id).await? {
                changes.items.push(item.into());
            }
        }
        for item_id in power_item_ids {
            if let Some(item) = items::get_power_item(db, self.player_id, item_id as u32).await? {
                changes.power_items.push(item.into());
            }
        }
        for item_id in insight_item_ids {
            if let Some(item) = items::get_insight_item(db, self.player_id, item_id as u32).await? {
                changes.insight_items.push(item.into());
            }
        }
        Ok(changes)
    }

    pub async fn equipment_snapshots(
        self,
        db: &SqlitePool,
        equip_uids: impl IntoIterator<Item = i64>,
    ) -> Result<Vec<Equip>, AppError> {
        let mut snapshots = Vec::new();
        for uid in equip_uids {
            snapshots.push(
                equipment_db::get_equipment_by_uid(db, self.player_id, uid)
                    .await?
                    .into(),
            );
        }
        Ok(snapshots)
    }

    pub async fn exchange_same_currency(
        self,
        db: &SqlitePool,
    ) -> Result<ExchangeSameCurrencyReply, AppError> {
        exchange_same_currency(db, self.player_id).await
    }

    pub async fn pop_exchange_same_currency(
        self,
        db: &SqlitePool,
        currency_ids: Vec<i32>,
    ) -> Result<PopExchangeSameCurrencyReply, AppError> {
        pop_exchange_same_currency(db, self.player_id, currency_ids).await
    }

    pub async fn exchange_diamond(
        self,
        db: &SqlitePool,
        amount: i32,
        op_type: i32,
    ) -> Result<ExchangeDiamondReply, AppError> {
        exchange_diamond(db, self.player_id, amount, op_type).await
    }

    pub async fn item_list(self, db: &SqlitePool) -> Result<GetItemListReply, AppError> {
        item_list(db, self.player_id).await
    }

    pub async fn buy_power_info(self, db: &SqlitePool) -> Result<GetBuyPowerInfoReply, AppError> {
        buy_power_info(db, self.player_id).await
    }

    pub async fn buy_power(self, db: &SqlitePool) -> Result<(BuyPowerReply, (i32, i32)), AppError> {
        buy_power(db, self.player_id).await
    }

    pub async fn auto_use_expired_power_items(
        self,
        db: &SqlitePool,
    ) -> Result<AutoUseExpirePowerItemReply, AppError> {
        auto_use_expired_power_items(db, self.player_id).await
    }

    pub async fn use_power_item(
        self,
        db: &SqlitePool,
        uid: i64,
    ) -> Result<(UsePowerItemReply, Vec<PowerItem>), AppError> {
        use_power_item(db, self.player_id, uid).await
    }

    pub async fn use_power_item_list(
        self,
        db: &SqlitePool,
        requested: Vec<UsePowerItemInfo>,
    ) -> Result<(UsePowerItemListReply, Vec<PowerItem>), AppError> {
        use_power_item_list(db, self.player_id, requested).await
    }

    pub async fn use_items(
        self,
        db: &SqlitePool,
        entries: Vec<M2qEntry>,
        target_id: Option<u64>,
    ) -> Result<
        (
            UseItemReply,
            reward::AppliedRewards,
            Vec<u32>,
            Vec<(u32, u32, i32)>,
        ),
        AppError,
    > {
        use_items(db, self.player_id, entries, target_id).await
    }

    pub async fn use_insight_item(
        self,
        db: &SqlitePool,
        uid: i64,
        hero_id: i32,
    ) -> Result<(UseInsightItemReply, i32), AppError> {
        use_insight_item(db, self.player_id, uid, hero_id).await
    }

    pub async fn mark_read_sub_type21(
        self,
        db: &SqlitePool,
        item_id: i32,
    ) -> Result<MarkReadSubType21Reply, AppError> {
        mark_read_sub_type21(db, self.player_id, item_id).await
    }

    pub async fn equip_info(self, db: &SqlitePool) -> Result<GetEquipInfoReply, AppError> {
        equipment::equip_info(db, self.player_id).await
    }

    pub async fn equip_lock(
        self,
        db: &SqlitePool,
        target_uid: i64,
        lock: bool,
    ) -> Result<EquipLockReply, AppError> {
        equipment::equip_lock(db, self.player_id, target_uid, lock).await
    }

    pub async fn strengthen_equip(
        self,
        db: &SqlitePool,
        target_uid: i64,
        eat_equips: Vec<EatEquip>,
    ) -> Result<equipment::StrengthenCompletion, AppError> {
        equipment::strengthen(db, self.player_id, target_uid, eat_equips).await
    }

    pub async fn break_equip(
        self,
        db: &SqlitePool,
        target_uid: i64,
    ) -> Result<(EquipBreakReply, Vec<(i32, i32)>, Vec<u32>, Vec<i64>), AppError> {
        equipment::break_equip(db, self.player_id, target_uid).await
    }

    pub async fn refine_equip(
        self,
        db: &SqlitePool,
        target_uid: i64,
        eat_uids: Vec<i64>,
    ) -> Result<(EquipRefineReply, Vec<i64>, Vec<i64>), AppError> {
        equipment::refine(db, self.player_id, target_uid, eat_uids).await
    }

    pub async fn decompose_equips(
        self,
        db: &SqlitePool,
        equip_uids: Vec<i64>,
    ) -> Result<(EquipDecomposeReply, Vec<i64>), AppError> {
        equipment::decompose(db, self.player_id, equip_uids).await
    }
}

#[cfg(test)]
mod tests;
