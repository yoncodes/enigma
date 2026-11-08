use crate::{error::AppError, reward, reward::RewardedReply};
use database::db::game::turnback;
use sonettobuf::{
    AcceptAllTurnbackBonusPointReply, BuyDoubleBonusReply, GetTurnbackDailyBonusReply,
    MaterialData, TurnbackBonusPointReply, TurnbackOnceBonusReply, TurnbackSignInReply,
};
use sqlx::SqlitePool;
use std::collections::BTreeMap;

pub(super) async fn turnback_once_bonus(
    db: &SqlitePool,
    player_id: i64,
    turnback_id: i32,
) -> Result<RewardedReply<TurnbackOnceBonusReply>, AppError> {
    let mut tx = db.begin().await?;
    let mut rewards = reward::RewardSet::default();
    if turnback::mark_once_bonus_in_transaction(&mut tx, player_id, turnback_id).await?
        && let Some(row) = config::configs::get().turnback.get(turnback_id)
    {
        rewards.extend(reward::parse(&row.once_bonus));
    }
    rewarded_reply_in_transaction(
        tx,
        db,
        player_id,
        TurnbackOnceBonusReply {
            id: Some(turnback_id),
        },
        rewards,
    )
    .await
}

pub(super) async fn turnback_sign_in(
    db: &SqlitePool,
    player_id: i64,
    turnback_id: i32,
    day: i32,
) -> Result<RewardedReply<TurnbackSignInReply>, AppError> {
    let mut tx = db.begin().await?;
    let mut rewards = reward::RewardSet::default();
    if turnback::claim_sign_in_in_transaction(&mut tx, player_id, turnback_id, day).await?
        && let Some(row) = config::configs::get()
            .turnback_sign_in
            .iter()
            .find(|row| row.turnback_id == turnback_id && row.day == day)
    {
        rewards.extend(reward::parse(&row.bonus));
    }
    rewarded_reply_in_transaction(
        tx,
        db,
        player_id,
        TurnbackSignInReply {
            id: Some(turnback_id),
            day: Some(day),
        },
        rewards,
    )
    .await
}

pub(super) async fn turnback_daily_bonus(
    db: &SqlitePool,
    player_id: i64,
    turnback_id: i32,
) -> Result<RewardedReply<GetTurnbackDailyBonusReply>, AppError> {
    let day = common::time::ServerTime::day_of_month(common::time::ServerTime::now_ms()) as i32;
    let mut tx = db.begin().await?;
    let (claimed, newly_claimed) =
        turnback::claim_daily_bonus_in_transaction(&mut tx, player_id, turnback_id, day).await?;
    let mut rewards = reward::RewardSet::default();
    if newly_claimed
        && let Some(row) = config::configs::get()
            .turnback_daily_bonus
            .iter()
            .find(|row| row.turnback_id == turnback_id && row.day == day)
    {
        rewards.extend(reward::parse(&row.bonus));
    }
    rewarded_reply_in_transaction(
        tx,
        db,
        player_id,
        GetTurnbackDailyBonusReply {
            id: Some(turnback_id),
            day: Some(claimed),
        },
        rewards,
    )
    .await
}

pub(super) async fn turnback_bonus_point(
    db: &SqlitePool,
    player_id: i64,
    turnback_id: i32,
    bonus_point_id: i32,
    tables: &config::GameDB,
) -> Result<RewardedReply<TurnbackBonusPointReply>, AppError> {
    let mut tx = db.begin().await?;
    let (has_get_task_bonus, newly_claimed, buy_double_bonus) =
        claim_turnback_bonus_ids(&mut tx, player_id, turnback_id, [bonus_point_id], tables).await?;
    let rewards = turnback_task_bonus_rewards(turnback_id, newly_claimed, buy_double_bonus, tables);
    rewarded_reply_in_transaction(
        tx,
        db,
        player_id,
        TurnbackBonusPointReply {
            id: Some(turnback_id),
            bonus_point_id: Some(bonus_point_id),
            has_get_task_bonus,
        },
        rewards,
    )
    .await
}

pub(super) async fn accept_all_turnback_bonus_point(
    db: &SqlitePool,
    player_id: i64,
    turnback_id: i32,
    tables: &config::GameDB,
) -> Result<RewardedReply<AcceptAllTurnbackBonusPointReply>, AppError> {
    let ids = tables
        .turnback_task_bonus
        .iter()
        .filter(|row| row.turnback_id == turnback_id)
        .map(|row| row.id)
        .collect::<Vec<_>>();
    let mut tx = db.begin().await?;
    let (has_get_task_bonus, newly_claimed, buy_double_bonus) =
        claim_turnback_bonus_ids(&mut tx, player_id, turnback_id, ids, tables).await?;
    let rewards = turnback_task_bonus_rewards(turnback_id, newly_claimed, buy_double_bonus, tables);
    rewarded_reply_in_transaction(
        tx,
        db,
        player_id,
        AcceptAllTurnbackBonusPointReply {
            id: Some(turnback_id),
            has_get_task_bonus,
        },
        rewards,
    )
    .await
}

pub(super) async fn buy_double_bonus(
    db: &SqlitePool,
    player_id: i64,
    turnback_id: i32,
) -> Result<RewardedReply<BuyDoubleBonusReply>, AppError> {
    let tables = config::configs::get();
    let mut tx = db.begin().await?;
    let state = turnback::get_active_state_in_transaction(&mut tx, player_id, tables)
        .await?
        .filter(|state| state.turnback_id == turnback_id)
        .ok_or(AppError::InvalidRequest)?;
    let claimed_ids = state.claimed_task_bonus_ids()?;
    let mut rewards = reward::RewardSet::default();
    if turnback::mark_buy_double_bonus_in_transaction(&mut tx, player_id, turnback_id).await?
        && let Some(row) = tables.turnback.get(turnback_id)
    {
        rewards.extend(reward::parse(&row.buy_bonus));
        for id in &claimed_ids {
            if let Some(bonus) = tables
                .turnback_task_bonus
                .get(*id)
                .filter(|bonus| bonus.turnback_id == turnback_id)
            {
                rewards.extend(reward::parse(&bonus.extra_bonus));
            }
        }
    }

    let material_changes = coalesced_material_changes(&rewards);
    let double_bonus = material_changes
        .iter()
        .map(|(materil_type, materil_id, quantity)| MaterialData {
            materil_type: Some(*materil_type),
            materil_id: Some(*materil_id),
            quantity: Some(*quantity),
        })
        .collect();
    let applied = if rewards.is_empty() {
        reward::AppliedRewards::default()
    } else {
        reward::apply_in_transaction(&mut tx, db, player_id, rewards).await?
    };
    tx.commit().await?;

    Ok(RewardedReply {
        reply: BuyDoubleBonusReply {
            id: Some(turnback_id),
            has_get_double_task_bonus: claimed_ids,
            double_bonus,
        },
        rewards: applied,
        material_changes,
    })
}

pub(super) fn coalesced_material_changes(rewards: &reward::RewardSet) -> Vec<(u32, u32, i32)> {
    rewards
        .material_changes()
        .into_iter()
        .fold(
            BTreeMap::<(u32, u32), i32>::new(),
            |mut totals, (kind, id, amount)| {
                *totals.entry((kind, id)).or_default() += amount;
                totals
            },
        )
        .into_iter()
        .map(|((kind, id), amount)| (kind, id, amount))
        .collect()
}

pub(super) async fn claim_turnback_bonus_ids(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    player_id: i64,
    turnback_id: i32,
    ids: impl IntoIterator<Item = i32>,
    tables: &config::GameDB,
) -> Result<(Vec<i32>, Vec<i32>, bool), AppError> {
    let Some(state) = turnback::get_active_state_in_transaction(tx, player_id, tables).await?
    else {
        return Ok((Vec::new(), Vec::new(), false));
    };
    if state.turnback_id != turnback_id {
        return Ok((
            state.claimed_task_bonus_ids()?,
            Vec::new(),
            state.buy_double_bonus,
        ));
    }

    let mut claimed = state
        .claimed_task_bonus_ids()?
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    let before = claimed.clone();

    for id in ids {
        if tables
            .turnback_task_bonus
            .get(id)
            .filter(|row| row.turnback_id == turnback_id && row.need_point <= state.bonus_point)
            .is_some()
        {
            claimed.insert(id);
        }
    }

    let claimed = claimed.into_iter().collect::<Vec<_>>();
    let newly_claimed = claimed
        .iter()
        .copied()
        .filter(|id| !before.contains(id))
        .collect::<Vec<_>>();
    let claimed = turnback::save_claimed_task_bonus_in_transaction(
        tx,
        player_id,
        turnback_id,
        &state.has_get_task_bonus,
        &claimed,
    )
    .await?
    .ok_or(AppError::InvalidRequest)?;
    Ok((claimed, newly_claimed, state.buy_double_bonus))
}

fn turnback_task_bonus_rewards(
    turnback_id: i32,
    ids: Vec<i32>,
    buy_double_bonus: bool,
    tables: &config::GameDB,
) -> reward::RewardSet {
    let mut rewards = reward::RewardSet::default();
    for id in ids {
        if let Some(row) = tables
            .turnback_task_bonus
            .get(id)
            .filter(|row| row.turnback_id == turnback_id)
        {
            rewards.extend(reward::parse(&row.bonus));
            if buy_double_bonus {
                rewards.extend(reward::parse(&row.extra_bonus));
            }
        }
    }

    rewards
}

pub(super) async fn rewarded_reply_in_transaction<T>(
    mut tx: sqlx::Transaction<'_, sqlx::Sqlite>,
    db: &SqlitePool,
    player_id: i64,
    reply: T,
    rewards: reward::RewardSet,
) -> Result<RewardedReply<T>, AppError> {
    let material_changes = rewards.material_changes();
    let rewards = if rewards.is_empty() {
        reward::AppliedRewards::default()
    } else {
        reward::apply_in_transaction(&mut tx, db, player_id, rewards).await?
    };
    tx.commit().await?;

    Ok(RewardedReply {
        reply,
        rewards,
        material_changes,
    })
}
