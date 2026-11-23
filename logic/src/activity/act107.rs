use super::*;

const ACTIVITY_TYPE_ID: i32 = 107;

pub async fn act107_goods_info(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
) -> Result<Get107GoodsInfoReply, AppError> {
    let activity_id = activity_id.ok_or(AppError::InvalidRequest)?;
    let tables = config::configs::get();
    tables
        .activity
        .get(activity_id)
        .filter(|activity| activity.type_id == ACTIVITY_TYPE_ID)
        .ok_or(AppError::InvalidRequest)?;

    let states =
        activity_state::get(db, player_id, activity_id, ActivityStateKind::Act107Goods).await?;
    let goods_infos = tables
        .activity107
        .iter()
        .filter(|goods| goods.activity_id == activity_id && goods.is_online)
        .map(|goods| Act107GoodsInfo {
            id: Some(goods.id),
            buy_count: Some(
                states
                    .get(&goods.id)
                    .map(|(_, progress, _)| *progress)
                    .unwrap_or_default(),
            ),
        })
        .collect::<Vec<_>>();
    if goods_infos.is_empty() {
        return Err(AppError::InvalidRequest);
    }

    Ok(Get107GoodsInfoReply {
        activity_id: Some(activity_id),
        goods_infos,
    })
}

pub struct Act107Purchase {
    pub reply: Buy107GoodsReply,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
}

pub async fn buy_act107_goods(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
    id: Option<i32>,
    num: Option<i32>,
) -> Result<Act107Purchase, AppError> {
    let activity_id = activity_id.ok_or(AppError::InvalidRequest)?;
    let id = id.ok_or(AppError::InvalidRequest)?;
    let num = num.filter(|num| *num > 0).ok_or(AppError::InvalidRequest)?;
    let tables = config::configs::get();
    tables
        .activity
        .get(activity_id)
        .filter(|activity| activity.type_id == ACTIVITY_TYPE_ID)
        .ok_or(AppError::InvalidRequest)?;
    let goods = tables
        .activity107
        .iter()
        .find(|goods| goods.activity_id == activity_id && goods.id == id && goods.is_online)
        .ok_or(AppError::InvalidRequest)?;

    let states =
        activity_state::get(db, player_id, activity_id, ActivityStateKind::Act107Goods).await?;
    let bought = states
        .get(&id)
        .map(|(_, progress, _)| *progress)
        .unwrap_or_default();
    let buy_count = bought.checked_add(num).ok_or(AppError::InvalidRequest)?;
    // No row sets preGoodsId yet; its unlock rule is unproven, so such goods stay refused.
    if (goods.max_buy_count > 0 && buy_count > goods.max_buy_count) || goods.pre_goods_id != 0 {
        return Err(AppError::InvalidRequest);
    }

    let mut cost = reward::parse(&goods.cost);
    cost.checked_scale(num)?;
    let mut product = reward::parse(&goods.product);
    product.checked_scale(num)?;
    let material_changes = product.material_changes();

    let mut tx = db.begin().await?;
    let consumed = reward::consume(&mut tx, player_id, &cost).await?;
    if !activity_state::advance_progress_in_transaction(
        &mut tx,
        player_id,
        activity_id,
        bought,
        ActivityStateSet {
            kind: ActivityStateKind::Act107Goods,
            entry_id: id,
            state: 0,
            progress: buy_count,
            ext: "",
        },
    )
    .await?
    {
        return Err(AppError::InvalidRequest);
    }
    let mut rewards = reward::apply_in_transaction(&mut tx, db, player_id, product).await?;
    tx.commit().await?;
    rewards.item_ids.extend(consumed.item_ids);
    rewards.currency_ids.extend(consumed.currency_ids);

    Ok(Act107Purchase {
        reply: Buy107GoodsReply {
            activity_id: Some(activity_id),
            goods_info: Some(Act107GoodsInfo {
                id: Some(id),
                buy_count: Some(buy_count),
            }),
            num: Some(num),
        },
        rewards,
        material_changes,
    })
}

#[cfg(test)]
mod tests;
