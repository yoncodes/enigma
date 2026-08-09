use super::*;

pub(super) async fn item_list(
    db: &SqlitePool,
    player_id: i64,
) -> Result<GetItemListReply, AppError> {
    let items = UserItemModel::new(player_id, db.clone());

    Ok(GetItemListReply {
        items: items
            .get_all_items()
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
        power_items: items
            .get_all_power_items()
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
        insight_items: items
            .get_all_insight_items()
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
        expire_items: Vec::new(),
        talent_items: Vec::new(),
    })
}

pub(super) async fn buy_power_info(
    db: &SqlitePool,
    player_id: i64,
) -> Result<GetBuyPowerInfoReply, AppError> {
    let max = power_max_buy_count()?;
    let (used, _) = player_infos::power_purchase_state(db, player_id).await?;

    Ok(GetBuyPowerInfoReply {
        can_buy_count: Some((max - used).max(0)),
    })
}

pub(super) async fn buy_power(
    db: &SqlitePool,
    player_id: i64,
) -> Result<(BuyPowerReply, (i32, i32)), AppError> {
    const POWER_BUY_COST_ID: i32 = 25;

    let max_buys = power_max_buy_count()?;
    let (used, level) = player_infos::power_purchase_state(db, player_id).await?;
    if used >= max_buys {
        return Err(AppError::InvalidRequest);
    }

    let costs = config::configs::get()
        .r#const
        .get(POWER_BUY_COST_ID)
        .ok_or(AppError::InvalidRequest)?
        .value
        .split('|')
        .collect::<Vec<_>>();
    let cost = costs
        .get(used as usize)
        .or_else(|| costs.last())
        .and_then(|value| {
            let mut parts = value.split('#');
            if parts.next()? != "2" {
                return None;
            }
            Some((
                parts.next()?.parse::<i32>().ok()?,
                parts.next()?.parse::<i32>().ok()?,
            ))
        })
        .ok_or(AppError::InvalidRequest)?;
    let stamina = config::configs::get()
        .player_level(level)
        .ok_or(AppError::InvalidRequest)?
        .add_buy_recover_power;
    let stamina_limit = config::configs::get()
        .currency
        .get(currencies::POWER_CURRENCY_ID)
        .ok_or(AppError::InvalidRequest)?
        .max_limit;

    match currencies::purchase_power(
        db,
        currencies::PowerPurchase {
            user_id: player_id,
            source_currency_id: cost.0,
            cost: cost.1,
            power_currency_id: currencies::POWER_CURRENCY_ID,
            power: stamina,
            power_limit: stamina_limit,
            expected_purchase_count: used,
            max_purchase_count: max_buys,
        },
    )
    .await?
    {
        currencies::LimitedExchangeResult::Applied => {}
        currencies::LimitedExchangeResult::InsufficientSource => {
            return Err(AppError::InsufficientCurrency);
        }
        currencies::LimitedExchangeResult::TargetLimit
        | currencies::LimitedExchangeResult::PurchaseLimit => {
            return Err(AppError::InvalidRequest);
        }
    }

    Ok((
        BuyPowerReply {
            can_buy_count: Some(max_buys - used - 1),
        },
        cost,
    ))
}

fn power_max_buy_count() -> Result<i32, AppError> {
    const POWER_MAX_BUY_COUNT_ID: i32 = 23;
    config::configs::get()
        .r#const
        .get(POWER_MAX_BUY_COUNT_ID)
        .and_then(|row| row.value.parse().ok())
        .ok_or(AppError::InvalidRequest)
}

pub(super) async fn auto_use_expired_power_items(
    db: &SqlitePool,
    player_id: i64,
) -> Result<AutoUseExpirePowerItemReply, AppError> {
    let now_seconds = ServerTime::now_ms() / 1000;
    let expired = items::get_expired_power_items(db, player_id, now_seconds).await?;

    if expired.is_empty() {
        return Ok(AutoUseExpirePowerItemReply { used: Some(false) });
    }

    let mut stamina = 0;
    for item in &expired {
        if let Some(row) = i32::try_from(item.item_id)
            .ok()
            .and_then(|item_id| config::configs::get().power_item.get(item_id))
        {
            stamina += row.effect * item.quantity;
        }
    }

    let uids = expired.iter().map(|item| item.uid).collect::<Vec<_>>();
    let stamina_limit = config::configs::get()
        .currency
        .get(currencies::POWER_CURRENCY_ID)
        .ok_or(AppError::InvalidRequest)?
        .max_limit;
    if !items::convert_expired_power_items_up_to_limit(
        db,
        player_id,
        &uids,
        currencies::POWER_CURRENCY_ID,
        stamina,
        stamina_limit,
    )
    .await?
    {
        return Err(AppError::InvalidRequest);
    }

    Ok(AutoUseExpirePowerItemReply { used: Some(true) })
}

pub(super) async fn use_power_item(
    db: &SqlitePool,
    player_id: i64,
    uid: i64,
) -> Result<(UsePowerItemReply, Vec<PowerItem>), AppError> {
    let updates = consume_power_items(db, player_id, &[(uid, 1)]).await?;
    Ok((UsePowerItemReply { uid: Some(uid) }, updates))
}

pub(super) async fn use_power_item_list(
    db: &SqlitePool,
    player_id: i64,
    requested: Vec<UsePowerItemInfo>,
) -> Result<(UsePowerItemListReply, Vec<PowerItem>), AppError> {
    let uses = requested
        .iter()
        .map(|item| {
            Ok((
                item.uid.ok_or(AppError::InvalidRequest)?,
                item.num.ok_or(AppError::InvalidRequest)?,
            ))
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let updates = consume_power_items(db, player_id, &uses).await?;
    Ok((
        UsePowerItemListReply {
            use_power_item_info: requested,
        },
        updates,
    ))
}

async fn consume_power_items(
    db: &SqlitePool,
    player_id: i64,
    uses: &[(i64, i32)],
) -> Result<Vec<PowerItem>, AppError> {
    let quantities = uses
        .iter()
        .try_fold(BTreeMap::new(), |mut total, (uid, num)| {
            if *uid <= 0 || *num <= 0 {
                return Err(AppError::InvalidRequest);
            }
            *total.entry(*uid).or_default() += num;
            Ok(total)
        })?;
    if quantities.is_empty() {
        return Err(AppError::InvalidRequest);
    }

    let now = ServerTime::now_ms() / 1000;
    let mut owned = Vec::with_capacity(quantities.len());
    let mut gained = 0i32;
    for (uid, num) in &quantities {
        let item = items::get_power_item_by_uid(db, player_id, *uid)
            .await?
            .ok_or(AppError::InvalidRequest)?;
        if item.quantity < *num || (item.expire_time > 0 && i64::from(item.expire_time) <= now) {
            return Err(AppError::InvalidRequest);
        }
        let effect = config::configs::get()
            .power_item
            .get(item.item_id as i32)
            .ok_or(AppError::InvalidRequest)?
            .effect;
        gained = gained
            .checked_add(effect.checked_mul(*num).ok_or(AppError::InvalidRequest)?)
            .ok_or(AppError::InvalidRequest)?;
        owned.push((item, *num));
    }

    let power = config::configs::get()
        .currency
        .get(currencies::POWER_CURRENCY_ID)
        .ok_or(AppError::InvalidRequest)?;
    let uses = owned
        .iter()
        .map(|(item, amount)| (item.uid, *amount))
        .collect::<Vec<_>>();
    let updates = items::consume_power_items_for_currency(
        db,
        player_id,
        &uses,
        currencies::POWER_CURRENCY_ID,
        gained,
        power.max_limit,
    )
    .await?
    .ok_or(AppError::InvalidRequest)?;

    Ok(updates.into_iter().map(Into::into).collect())
}
