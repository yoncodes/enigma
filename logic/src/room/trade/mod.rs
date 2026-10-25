use super::RoomManager;
use crate::{error::AppError, reward, reward::RewardedReply};
use database::db::game::{manufacture, room_orders, trade};
use sonettobuf::{
    GetOrderInfoReply, GetTradeSupportBonusReply, GetTradeTaskExtraBonusReply,
    GetTradeTaskInfoReply, ReadNewTradeTaskReply, TradeLevelUpReply,
};
use sqlx::SqlitePool;

impl RoomManager {
    pub async fn order_info(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
    ) -> Result<GetOrderInfoReply, AppError> {
        order_info(db, self.player_id, tables).await
    }

    pub async fn trade_task_info(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
    ) -> Result<GetTradeTaskInfoReply, AppError> {
        trade_task_info(db, self.player_id, tables).await
    }

    pub async fn read_new_trade_tasks(
        &self,
        db: &SqlitePool,
        ids: Vec<i32>,
    ) -> Result<ReadNewTradeTaskReply, AppError> {
        read_new_trade_task(db, self.player_id, ids).await
    }

    pub fn trade_task_extra_bonus(&self) -> GetTradeTaskExtraBonusReply {
        GetTradeTaskExtraBonusReply {}
    }

    pub async fn trade_support_bonus(
        &self,
        db: &SqlitePool,
        bonus_id: i32,
    ) -> Result<RewardedReply<GetTradeSupportBonusReply>, AppError> {
        get_trade_support_bonus(db, self.player_id, bonus_id).await
    }

    pub async fn trade_level_up(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
    ) -> Result<RewardedReply<TradeLevelUpReply>, AppError> {
        trade_level_up(db, self.player_id, tables).await
    }
}

async fn order_info(
    db: &SqlitePool,
    player_id: i64,
    tables: &config::GameDB,
) -> Result<GetOrderInfoReply, AppError> {
    Ok(room_orders::get_order_info(db, player_id, tables).await?)
}

async fn trade_task_info(
    db: &SqlitePool,
    player_id: i64,
    tables: &config::GameDB,
) -> Result<GetTradeTaskInfoReply, AppError> {
    let level = manufacture::get_trade_level(db, player_id, tables).await?;
    trade::sync_tasks(db, player_id, level, tables).await?;
    Ok(GetTradeTaskInfoReply {
        infos: trade::get_trade_tasks(db, player_id).await?,
        has_get_support_bonus: trade::get_support_bonus_ids(db, player_id).await?,
        can_get_extra_bonus: Some(false),
    })
}

async fn read_new_trade_task(
    db: &SqlitePool,
    player_id: i64,
    ids: Vec<i32>,
) -> Result<ReadNewTradeTaskReply, AppError> {
    trade::read_new_trade_tasks(db, player_id, &ids).await?;
    Ok(ReadNewTradeTaskReply { ids })
}

async fn get_trade_support_bonus(
    db: &SqlitePool,
    player_id: i64,
    bonus_id: i32,
) -> Result<RewardedReply<GetTradeSupportBonusReply>, AppError> {
    let bonus = config::configs::get()
        .trade_support_bonus
        .get(bonus_id)
        .ok_or(AppError::InvalidRequest)?;
    let finished_count = trade::finished_task_count(db, player_id).await?;
    if finished_count < bonus.need_task {
        return Err(AppError::InvalidRequest);
    }

    let mut tx = db.begin().await?;
    let claimed = trade::claim_support_bonus_in_transaction(&mut tx, player_id, bonus_id).await?;
    let rewards = if claimed {
        reward::parse(&bonus.bonus)
    } else {
        reward::RewardSet::default()
    };
    let material_changes = rewards.material_changes();
    let rewards = if rewards.is_empty() {
        reward::AppliedRewards::default()
    } else {
        reward::apply_in_transaction(&mut tx, db, player_id, rewards).await?
    };
    tx.commit().await?;

    Ok(RewardedReply {
        reply: GetTradeSupportBonusReply { id: Some(bonus_id) },
        rewards,
        material_changes,
    })
}

async fn trade_level_up(
    db: &SqlitePool,
    player_id: i64,
    tables: &config::GameDB,
) -> Result<RewardedReply<TradeLevelUpReply>, AppError> {
    let current = manufacture::get_trade_level(db, player_id, tables).await?;
    let next = current + 1;
    let level = tables
        .trade_level
        .iter()
        .find(|level| level.level == next)
        .ok_or(AppError::InvalidRequest)?;
    let finished_count = trade::finished_task_count(db, player_id).await?;
    if finished_count < level.level_up_need_task {
        return Err(AppError::InvalidRequest);
    }

    let mut tx = db.begin().await?;
    if !manufacture::set_trade_level_in_transaction(&mut tx, player_id, current, next).await? {
        return Err(AppError::InvalidRequest);
    }
    trade::sync_tasks_in_transaction(&mut tx, player_id, next, tables).await?;
    let rewards = reward::parse(&level.bonus);
    let material_changes = rewards.material_changes();
    let rewards = if rewards.is_empty() {
        reward::AppliedRewards::default()
    } else {
        reward::apply_in_transaction(&mut tx, db, player_id, rewards).await?
    };
    tx.commit().await?;

    Ok(RewardedReply {
        reply: TradeLevelUpReply { level: Some(next) },
        rewards,
        material_changes,
    })
}

#[cfg(test)]
mod tests;
