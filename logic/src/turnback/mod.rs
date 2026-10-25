mod claims;
mod info;

use crate::{error::AppError, reward::RewardedReply};
use sonettobuf::{
    AcceptAllTurnbackBonusPointReply, BuyDoubleBonusReply, GetTurnbackDailyBonusReply,
    GetTurnbackInfoReply, TurnbackBonusPointReply, TurnbackFirstShowReply, TurnbackOnceBonusReply,
    TurnbackSignInReply,
};
use sqlx::SqlitePool;

#[derive(Clone, Copy, Debug)]
pub struct TurnbackManager {
    player_id: i64,
}

impl TurnbackManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }

    pub async fn sync_state(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
    ) -> Result<(), AppError> {
        info::sync_state(db, self.player_id, tables).await
    }

    pub async fn info(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
    ) -> Result<GetTurnbackInfoReply, AppError> {
        info::turnback_info(db, self.player_id, tables).await
    }

    pub async fn mark_first_show(
        &self,
        db: &SqlitePool,
        turnback_id: i32,
    ) -> Result<TurnbackFirstShowReply, AppError> {
        info::turnback_first_show(db, self.player_id, turnback_id).await
    }

    pub async fn claim_once_bonus(
        &self,
        db: &SqlitePool,
        turnback_id: i32,
    ) -> Result<RewardedReply<TurnbackOnceBonusReply>, AppError> {
        claims::turnback_once_bonus(db, self.player_id, turnback_id).await
    }

    pub async fn claim_sign_in(
        &self,
        db: &SqlitePool,
        turnback_id: i32,
        day: i32,
    ) -> Result<RewardedReply<TurnbackSignInReply>, AppError> {
        claims::turnback_sign_in(db, self.player_id, turnback_id, day).await
    }

    pub async fn claim_daily_bonus(
        &self,
        db: &SqlitePool,
        turnback_id: i32,
    ) -> Result<RewardedReply<GetTurnbackDailyBonusReply>, AppError> {
        claims::turnback_daily_bonus(db, self.player_id, turnback_id).await
    }

    pub async fn claim_bonus_point(
        &self,
        db: &SqlitePool,
        turnback_id: i32,
        bonus_point_id: i32,
        tables: &config::GameDB,
    ) -> Result<RewardedReply<TurnbackBonusPointReply>, AppError> {
        claims::turnback_bonus_point(db, self.player_id, turnback_id, bonus_point_id, tables).await
    }

    pub async fn claim_all_bonus_points(
        &self,
        db: &SqlitePool,
        turnback_id: i32,
        tables: &config::GameDB,
    ) -> Result<RewardedReply<AcceptAllTurnbackBonusPointReply>, AppError> {
        claims::accept_all_turnback_bonus_point(db, self.player_id, turnback_id, tables).await
    }

    pub async fn buy_double_bonus(
        &self,
        db: &SqlitePool,
        turnback_id: i32,
    ) -> Result<RewardedReply<BuyDoubleBonusReply>, AppError> {
        claims::buy_double_bonus(db, self.player_id, turnback_id).await
    }
}

#[cfg(test)]
mod tests;
