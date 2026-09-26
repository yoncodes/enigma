use crate::error::AppError;
use crate::reward;
use chrono::Datelike;
use common::time::ServerTime;
use database::db::game::sign_in;
use sonettobuf::{
    GetSignInInfoReply, MaterialData, SignInAddupReply, SignInHistoryReply, SignInReply,
    SignInTotalRewardAllReply, SignInTotalRewardReply,
};
use sqlx::SqlitePool;

pub struct SignInAddupOutcome {
    pub reply: SignInAddupReply,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
}

pub struct SignInLifetimeRewardOutcome<T> {
    pub reply: T,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
}

pub struct SignInOutcome {
    pub reply: SignInReply,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
}

#[derive(Clone, Copy, Debug)]
pub struct SignInManager {
    player_id: i64,
}

impl SignInManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }

    pub async fn reset_counters(
        self,
        db: &SqlitePool,
        daily: bool,
        weekly: bool,
    ) -> Result<Vec<i32>, AppError> {
        let mut reset_task_ids = Vec::new();
        if weekly {
            reset_task_ids.extend(sign_in::reset_weekly_counters(db, self.player_id).await?);
        }
        if daily {
            reset_task_ids.extend(sign_in::reset_daily_counters(db, self.player_id).await?);
        }
        reset_task_ids.sort_unstable();
        reset_task_ids.dedup();
        Ok(reset_task_ids)
    }

    pub async fn get_info(&self, db: &SqlitePool) -> Result<GetSignInInfoReply, AppError> {
        let (info, sign_in_days, addup_bonus, month_card_days, month_card_history, birthday_heroes) =
            sign_in::get_sign_in_info(db, self.player_id).await?;

        Ok(GetSignInInfoReply {
            has_sign_in_days: sign_in_days,
            addup_sign_in_day: Some(info.addup_sign_in_day),
            has_get_addup_bonus: addup_bonus,
            open_function_time: Some(info.open_function_time),
            has_month_card_days: month_card_days,
            month_card_history: month_card_history.into_iter().map(Into::into).collect(),
            birthday_hero_ids: birthday_heroes,
            reward_mark: Some(info.reward_mark),
            supplement_month_card_days: Some(0),
        })
    }

    pub async fn sign_in(&self, db: &SqlitePool) -> Result<SignInOutcome, AppError> {
        let now = ServerTime::now_ms();
        let birthday_heroes =
            sign_in::get_claimed_birthday_heroes_today(db, self.player_id).await?;
        let mut tx = db.begin().await?;
        let was_new_sign_in =
            sign_in::record_sign_in_day_in_transaction(&mut tx, self.player_id, now).await?;
        let day = was_new_sign_in.then_some(ServerTime::day_of_month(now) as i32);
        let reward_set = if was_new_sign_in {
            let weekday = sign_in_bonus_id(now);
            config::configs::get()
                .sign_in_bonus
                .get(weekday)
                .map(|bonus| reward::parse(&bonus.signin_bonus))
                .unwrap_or_default()
        } else {
            Default::default()
        };
        let material_changes = reward_set.material_changes();
        let sign_in_reward = material_changes
            .iter()
            .map(|(materil_type, materil_id, quantity)| MaterialData {
                materil_type: Some(*materil_type),
                materil_id: Some(*materil_id),
                quantity: Some(*quantity),
            })
            .collect();
        let rewards = reward::apply_in_transaction(&mut tx, db, self.player_id, reward_set).await?;
        tx.commit().await?;

        Ok(SignInOutcome {
            reply: SignInReply {
                day,
                birthday_hero_ids: birthday_heroes,
                sign_in_reward,
                month_reward: Vec::new(),
            },
            rewards,
            material_changes,
        })
    }

    pub async fn history(
        &self,
        db: &SqlitePool,
        month: i32,
    ) -> Result<SignInHistoryReply, AppError> {
        let (_, sign_in_days, _, month_card_days, _, birthday_heroes) =
            sign_in::get_sign_in_info(db, self.player_id).await?;

        Ok(SignInHistoryReply {
            month: Some(month),
            has_sign_in_days: sign_in_days,
            has_month_card_days: month_card_days,
            birthday_hero_ids: birthday_heroes,
        })
    }

    pub async fn addup(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
        id: i32,
    ) -> Result<SignInAddupOutcome, AppError> {
        let Some(bonus) = tables.sign_in_addup_bonus.get(id) else {
            return Ok(SignInAddupOutcome::empty(id));
        };

        let mut tx = db.begin().await?;
        let addup_days =
            sign_in::addup_sign_in_days_in_transaction(&mut tx, self.player_id).await?;

        if addup_days < bonus.signinaddup {
            return Ok(SignInAddupOutcome::empty(id));
        }

        if !sign_in::claim_addup_bonus_in_transaction(&mut tx, self.player_id, id).await? {
            return Ok(SignInAddupOutcome::empty(id));
        }

        let reward_set = reward::parse(&bonus.signin_bonus);
        let material_changes = reward_set.material_changes();
        let rewards = reward::apply_in_transaction(&mut tx, db, self.player_id, reward_set).await?;
        tx.commit().await?;

        Ok(SignInAddupOutcome {
            reply: SignInAddupReply { id: Some(id) },
            rewards,
            material_changes,
        })
    }

    pub async fn total_reward(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
        id: i32,
    ) -> Result<SignInLifetimeRewardOutcome<SignInTotalRewardReply>, AppError> {
        let claim = claim_lifetime_rewards(db, tables, self.player_id, Some(id)).await?;
        Ok(SignInLifetimeRewardOutcome {
            reply: SignInTotalRewardReply {
                id: Some(id),
                mark: Some(claim.mark),
            },
            rewards: claim.rewards,
            material_changes: claim.material_changes,
        })
    }

    pub async fn total_reward_all(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
    ) -> Result<SignInLifetimeRewardOutcome<SignInTotalRewardAllReply>, AppError> {
        let claim = claim_lifetime_rewards(db, tables, self.player_id, None).await?;
        Ok(SignInLifetimeRewardOutcome {
            reply: SignInTotalRewardAllReply {
                mark: Some(claim.mark),
            },
            rewards: claim.rewards,
            material_changes: claim.material_changes,
        })
    }
}

fn sign_in_bonus_id(now: i64) -> i32 {
    ServerTime::adjusted_datetime(now)
        .weekday()
        .number_from_monday() as i32
}

struct LifetimeClaim {
    mark: i32,
    rewards: reward::AppliedRewards,
    material_changes: Vec<(u32, u32, i32)>,
}

async fn claim_lifetime_rewards(
    db: &SqlitePool,
    tables: &config::GameDB,
    player_id: i64,
    requested_id: Option<i32>,
) -> Result<LifetimeClaim, AppError> {
    let mut tx = db.begin().await?;
    sign_in::ensure_sign_in_info_in_transaction(&mut tx, player_id).await?;
    let (login_days, old_mark) =
        sign_in::lifetime_reward_state_in_transaction(&mut tx, player_id).await?;

    if requested_id.is_some()
        && !tables
            .sign_in_lifetime_bonus
            .iter()
            .any(|bonus| Some(bonus.stageid) == requested_id)
    {
        return Err(AppError::InvalidRequest);
    }

    let mut new_mark = old_mark;
    let mut reward_set = reward::RewardSet::default();
    for bonus in tables
        .sign_in_lifetime_bonus
        .iter()
        .filter(|bonus| requested_id.is_none() || requested_id == Some(bonus.stageid))
    {
        let Some(bit) = lifetime_reward_bit(bonus.stageid) else {
            continue;
        };
        if bonus.logindaysid <= login_days && old_mark & bit == 0 {
            new_mark |= bit;
            reward_set.extend(reward::parse(&bonus.bonus));
        }
    }

    if new_mark == old_mark {
        tx.commit().await?;
        return Ok(LifetimeClaim {
            mark: old_mark,
            rewards: reward::AppliedRewards::default(),
            material_changes: Vec::new(),
        });
    }

    if !sign_in::update_reward_mark_in_transaction(&mut tx, player_id, old_mark, new_mark).await? {
        tx.rollback().await?;
        let mark = sign_in::reward_mark(db, player_id).await?;
        return Ok(LifetimeClaim {
            mark,
            rewards: reward::AppliedRewards::default(),
            material_changes: Vec::new(),
        });
    }

    let material_changes = reward_set.material_changes();
    let rewards = reward::apply_in_transaction(&mut tx, db, player_id, reward_set).await?;
    tx.commit().await?;
    Ok(LifetimeClaim {
        mark: new_mark,
        rewards,
        material_changes,
    })
}

fn lifetime_reward_bit(id: i32) -> Option<i32> {
    u32::try_from(id)
        .ok()
        .and_then(|shift| 1_i32.checked_shl(shift))
}

impl SignInAddupOutcome {
    fn empty(day: i32) -> Self {
        Self {
            reply: SignInAddupReply { id: Some(day) },
            rewards: reward::AppliedRewards::default(),
            material_changes: Vec::new(),
        }
    }
}

#[cfg(test)]
mod test;
