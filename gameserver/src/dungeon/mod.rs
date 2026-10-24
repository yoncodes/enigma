use crate::{
    error::AppError,
    logic::{
        hero::HeroManager,
        reward::{self, AppliedRewards, ConsumedRewards},
    },
    player::battle::{ActiveBattle, PendingDungeonRecord},
    tower_compose,
};

use common::time::ServerTime;
use config::configs;
use database::db::game::{battle as battle_db, dungeons, instruction_dungeon, open_infos};
use sonettobuf::{
    AssistHeroCareerNo, AssistHeroInfo, DungeonUpdatePush, EndDungeonPush, EndFightPush,
    FightRecord, GetDungeonReply, GetPuzzleProgressReply, InstructionDungeonFinalRewardReply,
    InstructionDungeonInfoReply, InstructionDungeonOpenReply, InstructionDungeonRewardReply,
    MaterialData, PuzzleFinishReply, RefreshAssistReply, RefreshAssistRequest,
    SavePuzzleProgressReply,
};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::BTreeMap;

const TEACH_BOUNDS_CONFIG_ID: i32 = 1100;
const MAX_PUZZLE_PROGRESS_BYTES: usize = 64 * 1024;
mod assist;
mod info;
mod outcome;
mod progression;
mod record;
mod restore;
mod settlement;

pub use assist::*;
pub use info::*;
pub use outcome::*;
pub use progression::*;
pub use record::*;
pub use restore::*;
pub use settlement::*;

#[cfg(test)]
mod tests;
