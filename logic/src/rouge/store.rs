use std::collections::HashMap;

use chrono::{Duration, NaiveDate};
use database::db::game::rouge;
use sonettobuf::{Rouge2RewardInfo, Rouge2RewardReply};
use sqlx::SqlitePool;

use crate::{error::AppError, reward};

use super::RougeManager;

pub struct StoreResult {
    pub reply: Rouge2RewardReply,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
}

impl RougeManager {
    pub async fn buy_reward(
        self,
        db: &SqlitePool,
        reward_id: i32,
        amount: i32,
    ) -> Result<StoreResult, AppError> {
        self.buy_reward_at(
            db,
            reward_id,
            amount,
            common::time::ServerTime::now_sec_i32() as i64,
        )
        .await
    }

    pub(super) async fn buy_reward_at(
        self,
        db: &SqlitePool,
        reward_id: i32,
        amount: i32,
        now: i64,
    ) -> Result<StoreResult, AppError> {
        if amount <= 0 {
            return Err(AppError::InvalidRequest);
        }

        let tables = config::configs::get();
        let goods = tables
            .rouge2_reward
            .get(reward_id)
            .filter(|goods| goods.reward_score == 0 && goods.num > 0)
            .ok_or(AppError::InvalidRequest)?;
        let schedule = tables
            .rouge2_shop_date
            .get(goods.stage)
            .ok_or(AppError::InvalidRequest)?;
        let start = shop_time(&schedule.start_time).ok_or_else(|| {
            AppError::Custom(format!("invalid Rouge2 shop start time {}", schedule.id))
        })?;
        let end = shop_time(&schedule.end_time).ok_or_else(|| {
            AppError::Custom(format!("invalid Rouge2 shop end time {}", schedule.id))
        })?;
        if now < start || now >= end {
            return Err(AppError::InvalidRequest);
        }

        let cost = goods
            .num
            .checked_mul(amount)
            .ok_or(AppError::InvalidRequest)?;
        let mut product = reward::parse_strict(&goods.value)?;
        product.checked_scale(amount)?;
        let material_changes = product.material_changes();

        let mut tx = db.begin().await?;
        rouge::ensure_reward_buy_count(&mut tx, self.player_id, reward_id).await?;
        let counts = rouge::reward_buy_counts(&mut tx, self.player_id)
            .await?
            .into_iter()
            .collect::<HashMap<_, _>>();
        let buy_count = counts.get(&reward_id).copied().unwrap_or_default();
        let next_count = buy_count
            .checked_add(amount)
            .ok_or(AppError::InvalidRequest)?;
        if goods.max_buy_count > 0 {
            if next_count > goods.max_buy_count {
                return Err(AppError::InvalidRequest);
            }
        } else if tables
            .rouge2_reward
            .iter()
            .filter(|row| {
                row.stage == goods.stage && row.reward_score == 0 && row.max_buy_count > 0
            })
            .any(|row| counts.get(&row.id).copied().unwrap_or_default() < row.max_buy_count)
        {
            return Err(AppError::InvalidRequest);
        }

        let consumed = reward::consume(
            &mut tx,
            self.player_id,
            &reward::RewardSet {
                currencies: vec![(common::config().rouge2.store_currency_id, cost)],
                ..Default::default()
            },
        )
        .await?;
        let new_count =
            rouge::add_reward_buy_count(&mut tx, self.player_id, reward_id, buy_count, amount)
                .await?
                .ok_or(AppError::InvalidRequest)?;
        let mut rewards =
            reward::apply_in_transaction(&mut tx, db, self.player_id, product).await?;
        tx.commit().await?;
        rewards.item_ids.extend(consumed.item_ids);
        rewards.currency_ids.extend(consumed.currency_ids);

        Ok(StoreResult {
            reply: Rouge2RewardReply {
                goods_info: Some(Rouge2RewardInfo {
                    id: Some(reward_id),
                    buy_count: Some(new_count),
                }),
                num: Some(amount),
            },
            rewards,
            material_changes,
        })
    }
}

pub(super) fn shop_time(value: &str) -> Option<i64> {
    if let Some(timestamp) = common::time::ServerTime::config_datetime_sec(value) {
        return Some(i64::from(timestamp));
    }

    let parts = value
        .split(['-', ' ', ':'])
        .map(str::parse::<i32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let [year, month, day, hour, minute, second] = parts.as_slice() else {
        return None;
    };
    let month_index = i64::from(*year) * 12 + i64::from(*month) - 1;
    let year = i32::try_from(month_index.div_euclid(12)).ok()?;
    let month = u32::try_from(month_index.rem_euclid(12) + 1).ok()?;
    let date = NaiveDate::from_ymd_opt(year, month, 1)?
        .checked_add_signed(Duration::days(i64::from(*day) - 1))?;
    let local = date.and_hms_opt(
        u32::try_from(*hour).ok()?,
        u32::try_from(*minute).ok()?,
        u32::try_from(*second).ok()?,
    )?;
    Some(local.and_utc().timestamp() - common::time::SERVER_UTC_OFFSET_MS / 1_000)
}
