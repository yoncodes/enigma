use crate::{error::AppError, profile};
use database::{
    db::game::{
        antiques, battle_pass, block_packages, buildings, cloths, currencies, equipment, items,
        player_infos,
        tasks::{self as task_db},
    },
    models::game::{
        block_packages::{BlockPackage, SpecialBlock},
        buildings::Building,
        heros::UserHeroModel,
    },
};
use serde::{Deserialize, Serialize};
use sonettobuf::{AntiqueInfo, PlayerCloth};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::BTreeMap;

pub struct RewardedReply<T> {
    pub reply: T,
    pub rewards: AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
}

#[derive(Clone, Copy)]
pub enum RewardMaterialType {
    None = 0,
    Item = 1,
    Currency = 2,
    Exp = 3,
    Hero = 4,
    HeroSkin = 5,
    Faith = 6,
    PlayerCloth = 7,
    PlayerClothExp = 8,
    Equip = 9,
    PowerPotion = 10,
    Building = 11,
    Formula = 12,
    BlockPackage = 13,
    SpecialBlock = 14,
    Explore = 15,
    EquipCard = 16,
    Antique = 18,
    Season123EquipCard = 19,
    NewInsight = 24,
    Bp = 25,
    Act186Like = 26,
    Critter = 27,
    UnlockVoucher = 28,
    SpecialExpiredItem = 29,
    TalentItem = 31,
    RoomTheme = 1001,
    V1a5AiZiLa = 1002,
}

impl RewardMaterialType {
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::None),
            1 => Some(Self::Item),
            2 => Some(Self::Currency),
            3 => Some(Self::Exp),
            4 => Some(Self::Hero),
            5 => Some(Self::HeroSkin),
            6 => Some(Self::Faith),
            7 => Some(Self::PlayerCloth),
            8 => Some(Self::PlayerClothExp),
            9 => Some(Self::Equip),
            10 => Some(Self::PowerPotion),
            11 => Some(Self::Building),
            12 => Some(Self::Formula),
            13 => Some(Self::BlockPackage),
            14 => Some(Self::SpecialBlock),
            15 => Some(Self::Explore),
            16 => Some(Self::EquipCard),
            18 => Some(Self::Antique),
            19 => Some(Self::Season123EquipCard),
            24 => Some(Self::NewInsight),
            25 => Some(Self::Bp),
            26 => Some(Self::Act186Like),
            27 => Some(Self::Critter),
            28 => Some(Self::UnlockVoucher),
            29 => Some(Self::SpecialExpiredItem),
            31 => Some(Self::TalentItem),
            1001 => Some(Self::RoomTheme),
            1002 => Some(Self::V1a5AiZiLa),
            _ => None,
        }
    }

    pub const fn id(self) -> u32 {
        self as u32
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct RewardSet {
    pub player_exp: i32,
    pub items: Vec<(u32, i32)>,
    pub currencies: Vec<(i32, i32)>,
    pub block_packages: Vec<(i32, i32)>,
    pub heroes: Vec<(i32, i32)>,
    pub skins: Vec<(i32, i32)>,
    pub player_cloths: Vec<(i32, i32)>,
    pub player_cloth_exp: Vec<(i32, i32)>,
    pub equips: Vec<(i32, i32)>,
    pub power_items: Vec<(i32, i32)>,
    pub room_buildings: Vec<(i32, i32)>,
    pub special_blocks: Vec<(i32, i32)>,
    pub antiques: Vec<(i32, i32)>,
    pub insight_items: Vec<(i32, i32)>,
    pub bp_scores: Vec<(i32, i32)>,
}

impl RewardSet {
    pub fn extend(&mut self, other: Self) {
        self.player_exp = self.player_exp.saturating_add(other.player_exp);
        self.items.extend(other.items);
        self.currencies.extend(other.currencies);
        self.block_packages.extend(other.block_packages);
        self.heroes.extend(other.heroes);
        self.skins.extend(other.skins);
        self.player_cloths.extend(other.player_cloths);
        self.player_cloth_exp.extend(other.player_cloth_exp);
        self.equips.extend(other.equips);
        self.power_items.extend(other.power_items);
        self.room_buildings.extend(other.room_buildings);
        self.special_blocks.extend(other.special_blocks);
        self.antiques.extend(other.antiques);
        self.insight_items.extend(other.insight_items);
        self.bp_scores.extend(other.bp_scores);
    }

    pub fn is_empty(&self) -> bool {
        self.player_exp == 0
            && self.items.is_empty()
            && self.currencies.is_empty()
            && self.block_packages.is_empty()
            && self.heroes.is_empty()
            && self.skins.is_empty()
            && self.player_cloths.is_empty()
            && self.player_cloth_exp.is_empty()
            && self.equips.is_empty()
            && self.power_items.is_empty()
            && self.room_buildings.is_empty()
            && self.special_blocks.is_empty()
            && self.antiques.is_empty()
            && self.insight_items.is_empty()
            && self.bp_scores.is_empty()
    }

    pub fn scale(&mut self, multiplier: i32) {
        self.player_exp = self.player_exp.saturating_mul(multiplier);
        for rewards in [
            &mut self.currencies,
            &mut self.block_packages,
            &mut self.heroes,
            &mut self.skins,
            &mut self.player_cloths,
            &mut self.player_cloth_exp,
            &mut self.equips,
            &mut self.power_items,
            &mut self.room_buildings,
            &mut self.special_blocks,
            &mut self.antiques,
            &mut self.insight_items,
            &mut self.bp_scores,
        ] {
            for (_, count) in rewards {
                *count *= multiplier;
            }
        }
        for (_, count) in &mut self.items {
            *count *= multiplier;
        }
    }

    pub fn material_changes(&self) -> Vec<(u32, u32, i32)> {
        let mut changes = Vec::new();
        if self.player_exp != 0 {
            changes.push((RewardMaterialType::Exp.id(), 0, self.player_exp));
        }
        changes.extend(
            self.items
                .iter()
                .map(|(id, count)| (RewardMaterialType::Item.id(), *id, *count)),
        );
        changes.extend(
            self.currencies
                .iter()
                .map(|(id, count)| (RewardMaterialType::Currency.id(), *id as u32, *count)),
        );
        changes.extend(
            self.heroes
                .iter()
                .map(|(id, count)| (RewardMaterialType::Hero.id(), *id as u32, *count)),
        );
        changes.extend(
            self.skins
                .iter()
                .map(|(id, count)| (RewardMaterialType::HeroSkin.id(), *id as u32, *count)),
        );
        changes.extend(
            self.player_cloths
                .iter()
                .map(|(id, count)| (RewardMaterialType::PlayerCloth.id(), *id as u32, *count)),
        );
        changes.extend(
            self.player_cloth_exp
                .iter()
                .map(|(id, count)| (RewardMaterialType::PlayerClothExp.id(), *id as u32, *count)),
        );
        changes.extend(
            self.equips
                .iter()
                .map(|(id, count)| (RewardMaterialType::Equip.id(), *id as u32, *count)),
        );
        changes.extend(
            self.power_items
                .iter()
                .map(|(id, count)| (RewardMaterialType::PowerPotion.id(), *id as u32, *count)),
        );
        changes.extend(
            self.room_buildings
                .iter()
                .map(|(id, count)| (RewardMaterialType::Building.id(), *id as u32, *count)),
        );
        changes.extend(
            self.block_packages
                .iter()
                .map(|(id, count)| (RewardMaterialType::BlockPackage.id(), *id as u32, *count)),
        );
        changes.extend(
            self.special_blocks
                .iter()
                .map(|(id, count)| (RewardMaterialType::SpecialBlock.id(), *id as u32, *count)),
        );
        changes.extend(
            self.antiques
                .iter()
                .map(|(id, count)| (RewardMaterialType::Antique.id(), *id as u32, *count)),
        );
        changes.extend(
            self.insight_items
                .iter()
                .map(|(id, count)| (RewardMaterialType::NewInsight.id(), *id as u32, *count)),
        );
        changes.extend(
            self.bp_scores
                .iter()
                .map(|(id, count)| (RewardMaterialType::Bp.id(), *id as u32, *count)),
        );
        let mut combined = Vec::<(u32, u32, i32)>::new();
        for (kind, id, amount) in changes {
            if let Some((_, _, total)) =
                combined.iter_mut().find(|(existing_kind, existing_id, _)| {
                    *existing_kind == kind && *existing_id == id
                })
            {
                *total += amount;
            } else {
                combined.push((kind, id, amount));
            }
        }
        combined
    }
}

#[derive(Default)]
pub struct ConsumedRewards {
    pub item_ids: Vec<u32>,
    pub currency_ids: Vec<(i32, i32)>,
    pub material_changes: Vec<(u32, u32, i32)>,
}

pub(crate) async fn consume(
    tx: &mut Transaction<'_, Sqlite>,
    player_id: i64,
    costs: &RewardSet,
) -> Result<ConsumedRewards, AppError> {
    let mut consumed = ConsumedRewards::default();
    let item_costs = costs.items.iter().filter(|(_, amount)| *amount > 0).fold(
        BTreeMap::<u32, i32>::new(),
        |mut total, (id, amount)| {
            *total.entry(*id).or_default() += amount;
            total
        },
    );
    let currency_costs = costs
        .currencies
        .iter()
        .filter(|(_, amount)| *amount > 0)
        .fold(BTreeMap::<i32, i32>::new(), |mut total, (id, amount)| {
            *total.entry(*id).or_default() += amount;
            total
        });

    let now = common::time::ServerTime::now_ms();
    for (item_id, amount) in item_costs {
        if !items::consume_item_in_transaction(tx, player_id, item_id, amount, now).await? {
            return Err(AppError::InsufficientItems);
        }
        consumed.item_ids.push(item_id);
        consumed.material_changes.push((1, item_id, -amount));
    }

    for (currency_id, amount) in currency_costs {
        if !currencies::consume_currency_in_transaction(tx, player_id, currency_id, amount, now)
            .await?
        {
            return Err(AppError::InsufficientCurrency);
        }
        consumed.currency_ids.push((currency_id, -amount));
    }

    Ok(consumed)
}

pub struct SkinGain {
    pub skin_id: i32,
    pub first_gain: bool,
}

pub struct BpScoreGain {
    pub bp_id: i32,
    pub score: i32,
    pub weekly_score: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct RewardManager {
    player_id: i64,
}

impl RewardManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }

    pub async fn consume(
        self,
        tx: &mut Transaction<'_, Sqlite>,
        costs: &RewardSet,
    ) -> Result<ConsumedRewards, AppError> {
        consume(tx, self.player_id, costs).await
    }

    pub async fn apply(
        self,
        db: &SqlitePool,
        rewards: RewardSet,
    ) -> Result<AppliedRewards, AppError> {
        apply(db, self.player_id, rewards).await
    }

    pub async fn apply_in_transaction(
        self,
        tx: &mut Transaction<'_, Sqlite>,
        db: &SqlitePool,
        rewards: RewardSet,
    ) -> Result<AppliedRewards, AppError> {
        apply_in_transaction(tx, db, self.player_id, rewards).await
    }

    pub async fn apply_dungeon_in_transaction(
        self,
        tx: &mut Transaction<'_, Sqlite>,
        rewards: RewardSet,
    ) -> Result<AppliedRewards, AppError> {
        apply_dungeon_in_transaction(tx, self.player_id, rewards).await
    }
}

#[derive(Default)]
pub struct AppliedRewards {
    pub player_info_changed: bool,
    pub item_ids: Vec<u32>,
    pub currency_ids: Vec<(i32, i32)>,
    pub hero_ids: Vec<i32>,
    pub skin_gains: Vec<SkinGain>,
    pub cloth_updates: Vec<PlayerCloth>,
    pub equip_uids: Vec<i64>,
    pub power_item_ids: Vec<i32>,
    pub antiques: Vec<AntiqueInfo>,
    pub insight_item_ids: Vec<i32>,
    pub bp_scores: Vec<BpScoreGain>,
    pub room_buildings: Vec<Building>,
    pub block_packages: Vec<BlockPackage>,
    pub special_blocks: Vec<SpecialBlock>,
}

impl AppliedRewards {
    pub fn extend(&mut self, other: Self) {
        self.player_info_changed |= other.player_info_changed;
        self.item_ids.extend(other.item_ids);
        self.currency_ids.extend(other.currency_ids);
        self.hero_ids.extend(other.hero_ids);
        self.skin_gains.extend(other.skin_gains);
        self.cloth_updates.extend(other.cloth_updates);
        self.equip_uids.extend(other.equip_uids);
        self.power_item_ids.extend(other.power_item_ids);
        self.antiques.extend(other.antiques);
        self.insight_item_ids.extend(other.insight_item_ids);
        self.bp_scores.extend(other.bp_scores);
        self.room_buildings.extend(other.room_buildings);
        self.block_packages.extend(other.block_packages);
        self.special_blocks.extend(other.special_blocks);
    }
}
mod apply;
mod parse;

pub(crate) use apply::*;
pub use parse::*;

#[cfg(test)]
mod tests;
