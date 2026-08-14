use crate::{error::AppError, reward};
use chrono::{NaiveDateTime, TimeZone, Utc};
use database::db::game::{
    battle_pass,
    tasks::{self as task_db, TaskLoopType},
};
use sonettobuf::{
    BpBuyLevelReply, BpMarkFirstShowReply, BpScoreBonusInfo, GetAct233BpInfoReply, GetBpBonusReply,
    GetBpInfoReply, GetSelfSelectBonusReply, RedDotInfo, Task,
};
use sqlx::SqlitePool;
use std::collections::HashMap;

const BP_BUY_LEVEL_COST_CONFIG_ID: i32 = 111;
mod act233;
mod info;
mod progression;
mod rewards;
mod tasks;

pub use info::{BpBonusClaim, BpBonusRedDots, BpLevelPurchase, BpSelfSelectClaim};
pub(crate) use tasks::task_score_from_models;
use tasks::{bp_time_range, parse_bp_reward, score_bonus_info};
pub use tasks::{has_task_red_dot, task_score_from_tasks};

#[derive(Clone, Copy, Debug)]
pub struct BattlePassManager {
    player_id: i64,
}

impl BattlePassManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }

    pub async fn info(
        &self,
        db: &SqlitePool,
        include_tasks: bool,
    ) -> Result<GetBpInfoReply, AppError> {
        info::get_bp_info(db, self.player_id, include_tasks).await
    }

    pub async fn act233_info(
        &self,
        db: &SqlitePool,
        activity_id: Option<i32>,
        include_tasks: bool,
    ) -> Result<GetAct233BpInfoReply, AppError> {
        act233::get_info(db, self.player_id, activity_id, include_tasks).await
    }

    pub async fn claim_bonus(
        &self,
        db: &SqlitePool,
        id: Option<i32>,
        level: Option<i32>,
        pay_bonus: Option<bool>,
        is_sp: Option<bool>,
    ) -> Result<BpBonusClaim, AppError> {
        rewards::get_bp_bonus(db, self.player_id, id, level, pay_bonus, is_sp).await
    }

    pub async fn claim_self_select_bonus(
        &self,
        db: &SqlitePool,
        id: Option<i32>,
        level: Option<i32>,
        index: Option<i32>,
    ) -> Result<BpSelfSelectClaim, AppError> {
        rewards::get_self_select_bonus(db, self.player_id, id, level, index).await
    }

    pub async fn buy_levels(
        &self,
        db: &SqlitePool,
        id: Option<i32>,
        num: Option<i32>,
    ) -> Result<BpLevelPurchase, AppError> {
        progression::buy_levels(db, self.player_id, id, num).await
    }

    pub async fn mark_first_show(
        &self,
        db: &SqlitePool,
        id: Option<i32>,
        is_sp: Option<bool>,
    ) -> Result<BpMarkFirstShowReply, AppError> {
        progression::mark_first_show(db, self.player_id, id, is_sp).await
    }

    pub async fn bonus_red_dots(&self, db: &SqlitePool) -> Result<BpBonusRedDots, AppError> {
        info::bonus_red_dots(db, self.player_id).await
    }

    pub async fn task_red_dot_infos(&self, db: &SqlitePool) -> Result<Vec<RedDotInfo>, AppError> {
        tasks::task_red_dot_infos(db, self.player_id).await
    }
}

#[cfg(test)]
use info::bonus_red_dots_for_state;
#[cfg(test)]
use progression::level_purchase_cost;
#[cfg(test)]
use rewards::select_reward;
#[cfg(test)]
use tasks::{should_show_task_red_dot, task_tab_id};
#[cfg(test)]
mod test;
