use crate::{error::AppError, reward};
use database::{db::game::equipment, models::game::equipment::UserEquipmentModel};
use sonettobuf::{
    EatEquip, EquipBreakReply, EquipDecomposeReply, EquipLockReply, EquipRefineReply,
    EquipStrengthenReply, GetEquipInfoReply,
};
use sqlx::SqlitePool;
use std::collections::HashSet;

const DECOMPOSE_MAX_COUNT: usize = 100;
const EQUIPMENT_CURRENCY_ID: i32 = 3;

pub struct StrengthenCompletion {
    pub reply: EquipStrengthenReply,
    pub currency_changes: Vec<(i32, i32)>,
    pub changed_uids: Vec<i64>,
    pub deleted_uids: Vec<i64>,
}

pub(super) async fn equip_info(
    db: &SqlitePool,
    player_id: i64,
) -> Result<GetEquipInfoReply, AppError> {
    Ok(GetEquipInfoReply {
        equips: equipment::get_user_equipment(db, player_id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    })
}

pub(super) async fn equip_lock(
    db: &SqlitePool,
    player_id: i64,
    target_uid: i64,
    lock: bool,
) -> Result<EquipLockReply, AppError> {
    if !equipment::update_equipment_lock(db, player_id, target_uid, lock).await? {
        return Err(AppError::InvalidRequest);
    }

    Ok(EquipLockReply {
        target_uid: Some(target_uid),
        lock: Some(lock),
    })
}

pub(super) async fn strengthen(
    db: &SqlitePool,
    player_id: i64,
    target_uid: i64,
    eat_equips: Vec<EatEquip>,
) -> Result<StrengthenCompletion, AppError> {
    let Some(consumes) = eat_equips
        .iter()
        .map(|equip| Some((equip.eat_uid?, equip.count.unwrap_or(1))))
        .collect::<Option<Vec<_>>>()
    else {
        return Err(AppError::InvalidRequest);
    };
    if !valid_strengthen_consumes(&consumes) {
        return Err(AppError::InvalidRequest);
    }

    let tables = config::configs::get();
    let equipment_model = UserEquipmentModel::new(player_id, db.clone());
    let target = equipment_model
        .get_equip(target_uid)
        .await
        .map_err(|_| AppError::InvalidRequest)?;
    let target_config = tables
        .equip
        .get(target.equip_id)
        .ok_or(AppError::InvalidRequest)?;
    let max_level = tables
        .equip_break_cost(target_config.rare, target.break_lv)
        .map(|row| row.level)
        .ok_or(AppError::InvalidRequest)?;
    if target.level >= max_level {
        return Err(AppError::InvalidRequest);
    }

    let mut consume_plans = Vec::with_capacity(consumes.len());
    let mut added_exp = 0i32;
    for (uid, count) in &consumes {
        if *uid == target_uid {
            return Err(AppError::InvalidRequest);
        }
        let consumed = equipment_model
            .get_equip(*uid)
            .await
            .map_err(|_| AppError::InvalidRequest)?;
        if consumed.is_lock || *count > consumed.count {
            return Err(AppError::InvalidRequest);
        }
        let consumed_config = tables
            .equip
            .get(consumed.equip_id)
            .ok_or(AppError::InvalidRequest)?;
        let exp = incremental_exp(tables, &consumed, consumed_config)
            .ok_or(AppError::InvalidRequest)?
            .checked_mul(*count)
            .ok_or(AppError::InvalidRequest)?;
        added_exp = added_exp.checked_add(exp).ok_or(AppError::InvalidRequest)?;
        consume_plans.push(database::db::game::equipment::StrengthenConsume {
            uid: *uid,
            count: *count,
            expected_count: consumed.count,
            stackable: consumed_config.is_exp_equip == 1,
        });
    }
    let (level, exp, score_cost) = strengthened_level(
        tables,
        target_config.rare,
        max_level,
        target.level,
        target.exp,
        added_exp,
    )
    .ok_or(AppError::InvalidRequest)?;

    let costs = reward::RewardSet {
        currencies: (score_cost > 0)
            .then_some((EQUIPMENT_CURRENCY_ID, score_cost))
            .into_iter()
            .collect(),
        ..Default::default()
    };
    let mut tx = db.begin().await?;
    reward::consume(&mut tx, player_id, &costs).await?;
    if !database::db::game::equipment::apply_strengthen_in_transaction(
        &mut tx,
        player_id,
        equipment::StrengthenUpdate {
            target_uid,
            expected_level: target.level,
            expected_exp: target.exp,
            level,
            exp,
            consumes: &consume_plans,
        },
    )
    .await?
    {
        return Err(AppError::InvalidRequest);
    }
    tx.commit().await?;

    let mut changed_uids = vec![target_uid];
    let mut deleted_uids = Vec::new();
    for consume in consume_plans {
        if consume.expected_count > consume.count {
            changed_uids.push(consume.uid);
        } else {
            deleted_uids.push(consume.uid);
        }
    }

    Ok(StrengthenCompletion {
        reply: EquipStrengthenReply {
            target_uid: Some(target_uid),
            eat_equips,
        },
        currency_changes: (score_cost > 0)
            .then_some((EQUIPMENT_CURRENCY_ID, -score_cost))
            .into_iter()
            .collect(),
        changed_uids,
        deleted_uids,
    })
}

fn valid_strengthen_consumes(consumes: &[(i64, i32)]) -> bool {
    !consumes.is_empty()
        && consumes.iter().all(|(_, count)| *count > 0)
        && consumes
            .iter()
            .map(|(uid, _)| *uid)
            .collect::<HashSet<_>>()
            .len()
            == consumes.len()
}

fn incremental_exp(
    tables: &config::GameDB,
    equipment: &database::models::game::equipment::Equipment,
    equipment_config: &config::equip::Equip,
) -> Option<i32> {
    let base_exp = if equipment_config.is_exp_equip == 1 {
        config_pair(&tables.equip_const.get(1)?.value, equipment.equip_id)?
    } else {
        config_pair(&tables.equip_const.get(2)?.value, equipment_config.rare)?
    };
    if equipment_config.is_exp_equip == 1 || equipment.level == 1 {
        return Some(base_exp);
    }

    let transfers = &tables.equip_const.get(3)?.value;
    let mut hundredths = i64::from(base_exp) * 100;
    let mut start_level = 2;
    for break_level in 0..=equipment.break_lv {
        let break_max = tables
            .equip_break_cost(equipment_config.rare, break_level)?
            .level;
        let transfer = config_pair(transfers, break_level)?;
        for level in start_level..=equipment.level.min(break_max) {
            hundredths += i64::from(
                tables
                    .equip_strengthen_cost(equipment_config.rare, level)?
                    .exp,
            ) * i64::from(transfer);
        }
        start_level = break_max + 1;
    }

    let current_max = tables
        .equip_break_cost(equipment_config.rare, equipment.break_lv)?
        .level;
    let exp_transfer = if equipment.level < current_max {
        config_pair(transfers, equipment.break_lv)?
    } else {
        config_pair(transfers, equipment.break_lv + 1)
            .or_else(|| config_pair(transfers, equipment.break_lv))?
    };
    hundredths += i64::from(equipment.exp) * i64::from(exp_transfer);
    i32::try_from(hundredths / 100).ok()
}

fn strengthened_level(
    tables: &config::GameDB,
    rare: i32,
    max_level: i32,
    mut level: i32,
    mut exp: i32,
    mut added_exp: i32,
) -> Option<(i32, i32, i32)> {
    let mut score_cost = 0i32;
    while level < max_level && added_exp > 0 {
        let cost = tables.equip_strengthen_cost(rare, level + 1)?;
        let needed = cost.exp.saturating_sub(exp);
        let used = added_exp.min(needed);
        score_cost = score_cost.checked_add(
            i32::try_from(i64::from(used) * i64::from(cost.score_cost) / 1000).ok()?,
        )?;
        exp += used;
        added_exp -= used;
        if exp < cost.exp {
            break;
        }
        exp = 0;
        level += 1;
    }
    Some((level, exp, score_cost))
}

pub(super) async fn break_equip(
    db: &SqlitePool,
    player_id: i64,
    target_uid: i64,
) -> Result<(EquipBreakReply, Vec<(i32, i32)>, Vec<u32>, Vec<i64>), AppError> {
    let equips = UserEquipmentModel::new(player_id, db.clone());
    let target = equips.get_equip(target_uid).await?;
    let tables = config::configs::get();
    let equip_cfg = tables
        .equip
        .get(target.equip_id)
        .ok_or(AppError::InvalidRequest)?;
    let current = tables
        .equip_break_cost(equip_cfg.rare, target.break_lv)
        .ok_or(AppError::InvalidRequest)?;

    if target.level < current.level {
        return Ok((EquipBreakReply {}, Vec::new(), Vec::new(), Vec::new()));
    }

    let Some(next) = tables.equip_break_cost(equip_cfg.rare, target.break_lv + 1) else {
        return Ok((EquipBreakReply {}, Vec::new(), Vec::new(), Vec::new()));
    };

    let costs = reward::parse(&next.cost);
    let mut all_costs = costs.clone();
    if next.score_cost > 0 {
        all_costs
            .currencies
            .push((EQUIPMENT_CURRENCY_ID, next.score_cost));
    }
    let mut tx = db.begin().await?;
    match reward::consume(&mut tx, player_id, &all_costs).await {
        Ok(_) => {}
        Err(AppError::InsufficientItems | AppError::InsufficientCurrency) => {
            return Ok((EquipBreakReply {}, Vec::new(), Vec::new(), Vec::new()));
        }
        Err(error) => return Err(error),
    }
    if !equipment::advance_break_level_in_transaction(
        &mut tx,
        player_id,
        target_uid,
        target.break_lv,
    )
    .await?
    {
        return Ok((EquipBreakReply {}, Vec::new(), Vec::new(), Vec::new()));
    }
    tx.commit().await?;

    let changed_items = costs.items.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    let changed_currencies = if next.score_cost > 0 {
        vec![(EQUIPMENT_CURRENCY_ID, -next.score_cost)]
    } else {
        Vec::new()
    };
    let changed_uids = vec![target_uid];

    Ok((
        EquipBreakReply {},
        changed_currencies,
        changed_items,
        changed_uids,
    ))
}

pub(super) async fn refine(
    db: &SqlitePool,
    player_id: i64,
    target_uid: i64,
    eat_uids: Vec<i64>,
) -> Result<(EquipRefineReply, Vec<i64>, Vec<i64>), AppError> {
    if eat_uids.is_empty()
        || eat_uids.iter().copied().collect::<HashSet<_>>().len() != eat_uids.len()
    {
        return Err(AppError::InvalidRequest);
    }

    let tables = config::configs::get();
    let equips = UserEquipmentModel::new(player_id, db.clone());
    let target = equips
        .get_equip(target_uid)
        .await
        .map_err(|_| AppError::InvalidRequest)?;
    let target_config = tables
        .equip
        .get(target.equip_id)
        .ok_or(AppError::InvalidRequest)?;
    let universal_id = tables
        .equip_universal_refine_id()
        .ok_or(AppError::InvalidRequest)?;
    let max_level = tables
        .equip_max_refine_level()
        .ok_or(AppError::InvalidRequest)?;
    let rarity_threshold = tables
        .equip_refine_rarity_threshold()
        .ok_or(AppError::InvalidRequest)?;
    if target_config.is_exp_equip == 1
        || target.equip_id == universal_id
        || target_config.is_sp_refine == 1
        || target_config.rare <= rarity_threshold
        || target.refine_lv >= max_level
    {
        return Err(AppError::InvalidRequest);
    }
    let special_ids = target_config
        .use_sp_refine
        .split('#')
        .filter(|id| !id.is_empty())
        .map(str::parse)
        .collect::<Result<HashSet<i32>, _>>()
        .map_err(|_| AppError::InvalidRequest)?;

    let mut consumes = Vec::with_capacity(eat_uids.len());
    let mut level = target.refine_lv;
    for uid in &eat_uids {
        if level >= max_level {
            return Err(AppError::InvalidRequest);
        }
        let consumed = equips
            .get_equip(*uid)
            .await
            .map_err(|_| AppError::InvalidRequest)?;
        let consumed_config = tables
            .equip
            .get(consumed.equip_id)
            .ok_or(AppError::InvalidRequest)?;
        if *uid == target_uid
            || consumed.is_lock
            || consumed.refine_lv <= 0
            || !(consumed.equip_id == universal_id
                || (consumed.equip_id == target.equip_id && consumed_config.is_exp_equip != 1)
                || special_ids.contains(&consumed.equip_id))
        {
            return Err(AppError::InvalidRequest);
        }
        level = level
            .checked_add(consumed.refine_lv)
            .ok_or(AppError::InvalidRequest)?;
        consumes.push(equipment::RefineConsume {
            uid: *uid,
            equip_id: consumed.equip_id,
            refine_level: consumed.refine_lv,
        });
    }
    level = level.min(max_level);

    let mut tx = db.begin().await?;
    if !equipment::refine_equipment(
        &mut tx,
        player_id,
        target_uid,
        target.refine_lv,
        level,
        &consumes,
    )
    .await?
    {
        return Err(AppError::InvalidRequest);
    }
    tx.commit().await?;

    Ok((
        EquipRefineReply {
            target_uid: Some(target_uid),
            eat_uids: eat_uids.clone(),
        },
        vec![target_uid],
        eat_uids,
    ))
}

pub(super) async fn decompose(
    db: &SqlitePool,
    player_id: i64,
    equip_uids: Vec<i64>,
) -> Result<(EquipDecomposeReply, Vec<i64>), AppError> {
    if equip_uids.is_empty()
        || equip_uids.len() > DECOMPOSE_MAX_COUNT
        || equip_uids.iter().copied().collect::<HashSet<_>>().len() != equip_uids.len()
    {
        return Err(AppError::InvalidRequest);
    }

    let tables = config::configs::get();
    let equips = UserEquipmentModel::new(player_id, db.clone());
    let mut rarities = Vec::with_capacity(equip_uids.len());
    for uid in &equip_uids {
        let equip = equips
            .get_equip(*uid)
            .await
            .map_err(|_| AppError::InvalidRequest)?;
        let equip_cfg = tables
            .equip
            .get(equip.equip_id)
            .ok_or(AppError::InvalidRequest)?;
        if equip.is_lock
            || equip.level != 1
            || equip.count != 1
            || equip_cfg.rare >= 4
            || equip_cfg.is_exp_equip != 0
            || equip_cfg.is_sp_refine != 0
        {
            return Err(AppError::InvalidRequest);
        }
        rarities.push(equip_cfg.rare);
    }

    let base_exp = &tables
        .equip_const
        .get(2)
        .ok_or(AppError::InvalidRequest)?
        .value;
    let (output_equip_id, output_unit_count) = decompose_config(
        &tables
            .equip_const
            .get(17)
            .ok_or(AppError::InvalidRequest)?
            .value,
    )
    .ok_or(AppError::InvalidRequest)?;
    let output_count =
        decompose_count(base_exp, rarities, output_unit_count).ok_or(AppError::InvalidRequest)?;
    let changed_uids =
        equipment::decompose_equipment(db, player_id, &equip_uids, output_equip_id, output_count)
            .await?;

    Ok((EquipDecomposeReply { equip_uids }, changed_uids))
}

fn decompose_count(
    base_exp: &str,
    rarities: impl IntoIterator<Item = i32>,
    unit_count: i32,
) -> Option<i32> {
    let exp = rarities.into_iter().try_fold(0i32, |total, rarity| {
        total.checked_add(config_pair(base_exp, rarity)?)
    })?;
    (exp / 100)
        .checked_mul(unit_count)
        .filter(|count| *count > 0)
}

fn config_pair(value: &str, key: i32) -> Option<i32> {
    value.split('|').find_map(|entry| {
        let (entry_key, entry_value) = entry.split_once('#')?;
        if entry_key.parse::<i32>().ok()? == key {
            entry_value.parse().ok()
        } else {
            None
        }
    })
}

fn decompose_config(value: &str) -> Option<(i32, i32)> {
    let mut values = value.split('#').skip(1);
    Some((values.next()?.parse().ok()?, values.next()?.parse().ok()?))
}

#[cfg(test)]
mod test;
