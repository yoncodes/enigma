use crate::{error::AppError, reward};
use database::db::game::activity_state::{self, ActivityStateKind, ActivityStateSet};
use sonettobuf::{Act107GoodsInfo, Buy107GoodsReply, Get107GoodsInfoReply};
use sqlx::SqlitePool;

fn validate_activity(activity_id: i32) -> Result<(), AppError> {
    let activity = config::configs::get().activity.get(activity_id).ok_or(AppError::InvalidRequest)?;
    if activity.type_id != 107 || !config::activity107::goods()?.iter().any(|row| row.activity_id == activity_id) {
        return Err(AppError::InvalidRequest);
    }
    Ok(())
}

fn active_at(activity_id: i32, now: i64, schedule: &[common::config::ActivitySchedule]) -> bool {
    let Some(activity) = config::configs::get().activity.get(activity_id) else { return false };
    if activity.open_id != 0 && !config::configs::get().open.get(activity.open_id).is_some_and(|row| row.is_online != 0) {
        return false;
    }
    let Ok(now) = u64::try_from(now) else { return false };
    schedule.iter().find(|row| row.id == activity_id)
        .is_some_and(|row| row.is_unlock && row.start_time <= now && now <= row.end_time)
}

pub(super) async fn infos(db: &SqlitePool, player_id: i64, activity_id: i32) -> Result<Get107GoodsInfoReply, AppError> {
    validate_activity(activity_id)?;
    let states = activity_state::get(db, player_id, activity_id, ActivityStateKind::Act107Goods).await?;
    let goods_infos = config::activity107::goods()?.iter().filter(|row| row.activity_id == activity_id && row.is_online)
        .map(|row| Act107GoodsInfo { id: Some(row.id), buy_count: Some(states.get(&row.id).map_or(0, |v| v.0)) }).collect();
    Ok(Get107GoodsInfoReply { activity_id: Some(activity_id), goods_infos })
}

fn scale_checked(rewards: &mut reward::RewardSet, num: i32) -> Result<(), AppError> {
    rewards.player_exp = rewards.player_exp.checked_mul(num).ok_or(AppError::InvalidRequest)?;
    for entries in [&mut rewards.currencies, &mut rewards.block_packages, &mut rewards.heroes,
        &mut rewards.skins, &mut rewards.player_cloths, &mut rewards.player_cloth_exp,
        &mut rewards.equips, &mut rewards.power_items, &mut rewards.room_buildings,
        &mut rewards.special_blocks, &mut rewards.antiques, &mut rewards.insight_items, &mut rewards.bp_scores] {
        for (_, count) in entries { *count = count.checked_mul(num).ok_or(AppError::InvalidRequest)?; }
    }
    for (_, count) in &mut rewards.items { *count = count.checked_mul(num).ok_or(AppError::InvalidRequest)?; }
    Ok(())
}

pub(super) async fn buy(db: &SqlitePool, player_id: i64, activity_id: i32, goods_id: i32, num: i32, now: i64)
    -> Result<reward::RewardedReply<Buy107GoodsReply>, AppError> {
    buy_with_schedule(db, player_id, activity_id, goods_id, num, now, common::activity_schedule()).await
}

async fn buy_with_schedule(db: &SqlitePool, player_id: i64, activity_id: i32, goods_id: i32, num: i32, now: i64,
    schedule: &[common::config::ActivitySchedule]) -> Result<reward::RewardedReply<Buy107GoodsReply>, AppError> {
    validate_activity(activity_id)?;
    if !(1..=10_000).contains(&num) || !active_at(activity_id, now, schedule) { return Err(AppError::InvalidRequest); }
    let goods = config::activity107::find(activity_id, goods_id)?.filter(|row| row.is_online).ok_or(AppError::InvalidRequest)?;
    let mut costs = reward::parse_strict(&goods.cost)?;
    // All online prices in this client snapshot are one positive currency amount.
    // Reject other shapes rather than accidentally giving away an unsupported cost.
    let mut non_currency = costs.clone();
    non_currency.currencies.clear();
    if costs.currencies.len() != 1 || costs.currencies[0].0 <= 0 || costs.currencies[0].1 <= 0 || !non_currency.is_empty() {
        return Err(AppError::InvalidRequest);
    }
    let mut product = reward::parse_strict(&goods.product)?;
    if product.is_empty() { return Err(AppError::InvalidRequest); }
    scale_checked(&mut costs, num)?;
    scale_checked(&mut product, num)?;
    let material_changes = product.material_changes();

    let mut tx = db.begin().await?;
    let count: Option<i32> = sqlx::query_scalar("SELECT state FROM user_activity_state WHERE user_id = ? AND activity_id = ? AND kind = ? AND entry_id = ?")
        .bind(player_id).bind(activity_id).bind(ActivityStateKind::Act107Goods.id()).bind(goods_id)
        .fetch_optional(&mut *tx).await?;
    let count = count.unwrap_or(0);
    if count < 0 { return Err(AppError::InvalidRequest); }
    let next_count = count.checked_add(num).ok_or(AppError::InvalidRequest)?;
    if goods.max_buy_count > 0 && next_count > goods.max_buy_count { return Err(AppError::InvalidRequest); }
    if goods.pre_goods_id > 0 {
        let previous: Option<i32> = sqlx::query_scalar("SELECT state FROM user_activity_state WHERE user_id = ? AND activity_id = ? AND kind = ? AND entry_id = ?")
            .bind(player_id).bind(activity_id).bind(ActivityStateKind::Act107Goods.id()).bind(goods.pre_goods_id)
            .fetch_optional(&mut *tx).await?;
        if previous.unwrap_or(0) <= 0 { return Err(AppError::InvalidRequest); }
    }
    let consumed = reward::consume(&mut tx, player_id, &costs).await?;
    let transitioned = activity_state::transition_in_transaction(&mut tx, player_id, activity_id, count,
        ActivityStateSet { kind: ActivityStateKind::Act107Goods, entry_id: goods_id, state: next_count, progress: 0, ext: "" }).await?;
    if !transitioned { return Err(AppError::InvalidRequest); }
    let mut rewards = reward::apply_in_transaction(&mut tx, db, player_id, product).await?;
    tx.commit().await?;
    rewards.item_ids.extend(consumed.item_ids);
    rewards.currency_ids.extend(consumed.currency_ids);
    Ok(reward::RewardedReply {
        reply: Buy107GoodsReply { activity_id: Some(activity_id), goods_info: Some(Act107GoodsInfo { id: Some(goods_id), buy_count: Some(next_count) }), num: Some(num) },
        rewards,
        material_changes,
    })
}

#[cfg(test)]
mod tests;
