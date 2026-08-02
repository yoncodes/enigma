use crate::{
    error::AppError,
    reward::{self, ConsumedRewards},
};
use database::db::game::player_infos;
use database::models::game::heros::{HeroData, HeroModel, UserHeroModel};
use sonettobuf::{
    CancelHero3124TalentTreeReply, ChoiceHero3123WeaponReply, ChoiceHero3124TalentTreeReply,
    DestinyLevelUpReply, DestinyRankUpReply, DestinyStoneUnlockReply, DestinyStoneUseReply,
    GetHeroBirthdayReply, HeroDefaultEquipReply, HeroInfo, HeroLevelUpReply, HeroRankUpReply,
    HeroRedDotReadReply, HeroTouchReply, HeroUpgradeSkillReply, ItemUnlockReply,
    MarkHeroFavorReply, ResetHero3124TalentTreeReply, UnMarkIsNewReply, UnlockVoiceReply,
    UseSkinReply,
};
use sqlx::{Sqlite, SqlitePool, Transaction};

use std::collections::{BTreeMap, BTreeSet};

enum HeroConstId {
    TouchFaith = 33,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum UniqueSkillKind {
    Weapon = 2,
    TalentTree = 3,
}

mod destiny;
mod group;
mod profile;
mod progression;
mod specialization;
mod talent;

pub use destiny::{destiny_available, destiny_stones};
pub(crate) use profile::snapshot_data;
pub(crate) use profile::snapshot_data as snapshot;

#[derive(Clone, Copy, Debug)]
pub struct HeroManager {
    player_id: i64,
}

#[derive(Clone, Copy, Debug)]
pub struct DestinyMaterialTarget {
    pub rank: i32,
    pub stone_id: i32,
}

impl HeroManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }
}

#[cfg(test)]
use destiny::next_destiny_slot;
#[cfg(test)]
use progression::duplicate_item_id;
#[cfg(test)]
use specialization::{hero_3124_talent_id, update_talent_extra_str};

#[cfg(test)]
mod test;
