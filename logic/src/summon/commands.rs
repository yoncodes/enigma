use super::*;

enum SummonConstId {
    SinglePullCurrencyCost = 70,
}

pub(super) async fn summon_info(
    db: &SqlitePool,
    player_id: i64,
) -> Result<GetSummonInfoReply, AppError> {
    let stats = summon::get_summon_stats(db, player_id).await?;
    let mut pool_infos = summon::get_summon_pool_infos(db, player_id).await?;
    let tables = config::configs::get();
    for info in &mut pool_infos {
        let Some(pool) = tables.summon_pool.get(info.pool.pool_id) else {
            continue;
        };
        if !SummonType::from(pool.r#type).uses_sp_pool_info() {
            continue;
        }
        summon::ensure_sp_pool_info(db, player_id, pool.id, pool.r#type).await?;
        if let Some(sp_pool) = &mut info.sp_pool {
            sp_pool.sp_type = pool.r#type;
        } else {
            info.sp_pool = Some(database::models::game::summon::SpPoolInfo {
                sp_type: pool.r#type,
                up_hero_ids: Vec::new(),
                limited_ticket_id: 0,
                limited_ticket_num: 0,
                open_time: 0,
                used_first_ssr_guarantee: false,
                has_get_reward_progresses: Vec::new(),
                infallible_item_status: 0,
            });
        }
    }

    let newbie = pool_infos.iter().find_map(|info| {
        let pool = tables.summon_pool.get(info.pool.pool_id)?;
        is_newbie_pool(pool).then_some((pool, info.pool.summon_count))
    });
    let (is_show_new_summon, new_summon_count) = if let Some((pool, count)) = newbie {
        let limit = newbie_pull_limit(pool)?;
        (count < limit, count.min(limit))
    } else {
        (stats.is_show_new_summon, stats.new_summon_count)
    };

    Ok(GetSummonInfoReply {
        free_equip_summon: Some(stats.free_equip_summon),
        is_show_new_summon: Some(is_show_new_summon),
        new_summon_count: Some(new_summon_count),
        pool_infos: pool_infos.into_iter().map(Into::into).collect(),
        total_summon_count: Some(stats.total_summon_count),
    })
}

pub(super) async fn progress_rewards(
    db: &SqlitePool,
    player_id: i64,
    pool_id: i32,
) -> Result<(GetSummonProgressRewardsReply, Vec<u32>), AppError> {
    let tables = config::configs::get();
    let pool = tables
        .summon_pool
        .iter()
        .find(|pool| pool.id == pool_id && !pool.progress_rewards.is_empty())
        .ok_or(AppError::InvalidRequest)?;
    let rewards = pool
        .progress_rewards
        .split('|')
        .map(|entry| {
            let (progress, hero_id) = entry.split_once('#').ok_or(AppError::InvalidRequest)?;
            let progress = progress.parse().map_err(|_| AppError::InvalidRequest)?;
            let hero_id = hero_id.parse().map_err(|_| AppError::InvalidRequest)?;
            let item = tables
                .character
                .get(hero_id)
                .map(|hero| reward::parse(&hero.duplicate_item))
                .and_then(|reward| reward.items.into_iter().next())
                .ok_or(AppError::InvalidRequest)?;
            Ok((progress, item.0 as i32, item.1))
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let (has_get_reward_progresses, changed_items) =
        summon::claim_progress_rewards(db, player_id, pool_id, &rewards).await?;

    Ok((
        GetSummonProgressRewardsReply {
            pool_id: Some(pool_id),
            has_get_reward_progresses,
        },
        changed_items.into_iter().map(|id| id as u32).collect(),
    ))
}

pub(super) async fn pop_up_recommend_window(
    db: &SqlitePool,
    player_id: i64,
    pool_id: i32,
    order_id: i32,
) -> Result<PopUpRecommendWindowReply, AppError> {
    config::configs::get()
        .summon_pool_package
        .iter()
        .find(|row| row.id == pool_id && row.order == order_id && row.package_recommend_switch)
        .ok_or(AppError::InvalidRequest)?;
    let count = summon::increment_recommend_pop_up_count(db, player_id, pool_id, order_id).await?;

    Ok(PopUpRecommendWindowReply {
        pool_id: Some(pool_id),
        order_id: Some(order_id),
        pop_up_count: Some(count),
    })
}

pub(super) async fn choose_enhanced_pool_hero(
    db: &SqlitePool,
    player_id: i64,
    pool_id: i32,
    hero_id: i32,
) -> Result<ChooseEnhancedPoolHeroReply, AppError> {
    summon::update_sp_pool_up_heroes(db, player_id, pool_id, vec![hero_id]).await?;

    Ok(ChooseEnhancedPoolHeroReply {
        pool_id: Some(pool_id),
        hero_id: Some(hero_id),
    })
}

pub(super) async fn choose_multi_up_hero(
    db: &SqlitePool,
    player_id: i64,
    pool_id: i32,
    hero_ids: Vec<i32>,
) -> Result<ChooseMultiUpHeroReply, AppError> {
    if hero_ids.is_empty() {
        return Err(AppError::InvalidRequest);
    }

    summon::update_sp_pool_up_heroes(db, player_id, pool_id, hero_ids.clone()).await?;

    Ok(ChooseMultiUpHeroReply {
        pool_id: Some(pool_id),
        hero_ids,
    })
}

pub(super) async fn query_token(
    db: &SqlitePool,
    player_id: i64,
) -> Result<(SummonQueryTokenReply, EndActivityPush), AppError> {
    let token = account::get_user_token(db, player_id).await?;
    let activity_id = config::configs::get()
        .activity
        .iter()
        .filter(|activity| activity.type_id == 172)
        .map(|activity| activity.id)
        .max()
        .ok_or(AppError::InvalidRequest)?;

    Ok((
        SummonQueryTokenReply {
            token: Some(token.token),
        },
        EndActivityPush {
            id: Some(activity_id as u32),
        },
    ))
}

pub(super) async fn summon(
    db: &SqlitePool,
    player_id: i64,
    pool_id: i32,
    guide_id: Option<i32>,
    step_id: Option<i32>,
    count: i32,
) -> Result<SummonCompletion, AppError> {
    validate_summon_count(count)?;
    let tables = config::configs::get();
    let pool_cfg = tables
        .summon_pool
        .iter()
        .find(|pool| pool.id == pool_id)
        .ok_or(AppError::InvalidRequest)?;
    let is_newbie_pool = is_newbie_pool(pool_cfg);
    let newbie_count = if is_newbie_pool {
        summon::get_summon_count(db, player_id, pool_id).await?
    } else {
        0
    };
    let newbie_limit = is_newbie_pool
        .then(|| newbie_pull_limit(pool_cfg))
        .transpose()?;
    if newbie_limit.is_some_and(|limit| newbie_count + count > limit) {
        return Err(AppError::InvalidRequest);
    }
    let cost = if count == 10 {
        pool_cfg.cost10.clone()
    } else {
        pool_cfg.cost1.clone()
    };
    let cost = select_summon_cost(db, player_id, cost).await?;

    let sp_pool = summon::get_sp_pool_info(db, player_id, pool_id).await?;
    let expected_gacha = summon::get_gacha_state(db, player_id, pool_id).await?;
    let (pity_6, up_guaranteed) = expected_gacha.unwrap_or_default();
    let mut gacha = GachaState {
        pity_6,
        up_guaranteed,
    };
    let teaching = guide_id
        .zip(step_id)
        .filter(|_| count == 1)
        .and_then(|(guide_id, step_id)| {
            tables
                .teaching_summon
                .get(guide_id)
                .filter(|rule| rule.step_id == step_id && rule.pool_id == pool_id)
        });
    let results = if let Some(teaching) = teaching {
        let progress = guides::get_guide_progress(db, player_id, teaching.id)
            .await?
            .filter(|progress| progress.step_id == teaching.previous_step_id)
            .ok_or(AppError::InvalidRequest)?;
        gacha.pity_6 = gacha.pity_6.saturating_add(1);
        (
            vec![GachaResult {
                hero_id: teaching.hero_id,
            }],
            Some(progress.step_id),
        )
    } else {
        let pool = build_gacha_pool(pool_id, sp_pool.as_ref())?;
        let rules = GachaRules::from_pool(pool_cfg)?;
        let mut rng = rand::rng();
        let results = if count == 10 {
            gacha.ten_pull(&rules, &pool, &mut rng)
        } else {
            vec![gacha.single_pull(&rules, &pool, &mut rng, false)]
        };
        (results, None)
    };
    let completed_newbie_pool = newbie_limit.is_some_and(|limit| newbie_count + count == limit);

    let heroes = UserHeroModel::new(player_id, db.clone());
    let mut reply_results = Vec::new();
    let mut changed = reward::AppliedRewards {
        item_ids: Vec::new(),
        currency_ids: Vec::new(),
        hero_ids: Vec::new(),
        skin_gains: Vec::new(),
        equip_uids: Vec::new(),
        power_item_ids: Vec::new(),
        insight_item_ids: Vec::new(),
        bp_scores: Vec::new(),
        ..Default::default()
    };

    let mut tx = db.begin().await?;
    let consumed = reward::consume(&mut tx, player_id, &cost).await?;
    for result in results.0 {
        let grant = heroes
            .grant_hero_in_transaction(&mut tx, result.hero_id)
            .await?;
        changed.hero_ids.push(result.hero_id);
        if !grant.is_new && grant.duplicate_count > 0 {
            let dupe = reward::hero_duplicate_rewards(result.hero_id, grant.duplicate_count)?;
            let applied = reward::apply_in_transaction(&mut tx, db, player_id, dupe).await?;
            changed.extend(applied);
        }

        reply_results.push(SummonResult {
            hero_id: Some(result.hero_id),
            is_new: Some(grant.is_new),
            duplicate_count: Some(grant.duplicate_count),
            equip_id: Some(0),
            return_materials: Vec::new(),
            lucky_bag_id: Some(0),
            limited_ticket_id: Some(0),
        });
    }

    if pool_cfg.ticket_id != 0 {
        database::db::game::items::add_item_in_transaction(
            &mut tx,
            player_id,
            pool_cfg.ticket_id as u32,
            count,
            common::time::ServerTime::now_ms(),
        )
        .await?;
        changed.item_ids.push(pool_cfg.ticket_id as u32);
    }

    if !summon::save_gacha_state_in_transaction(
        &mut tx,
        player_id,
        pool_id,
        expected_gacha,
        gacha.pity_6,
        gacha.up_guaranteed,
    )
    .await?
    {
        return Err(AppError::InvalidRequest);
    }
    let guide_info = if let (Some(teaching), Some(expected_step)) = (teaching, results.1) {
        if !guides::update_guide_progress_in_transaction(
            &mut tx,
            player_id,
            teaching.id,
            Some(expected_step),
            teaching.step_id,
        )
        .await?
        {
            return Err(AppError::InvalidRequest);
        }
        Some(GuideInfo {
            guide_id: teaching.id,
            step_id: teaching.step_id,
        })
    } else {
        None
    };
    summon::increment_summon_count(&mut tx, player_id, pool_id, count).await?;
    summon::record_summon(
        &mut tx,
        player_id,
        count,
        is_newbie_pool,
        completed_newbie_pool,
    )
    .await?;
    summon::add_summon_history_in_transaction(
        &mut tx,
        player_id,
        pool_id,
        &pool_cfg.name_en,
        pool_cfg.r#type,
        if count == 10 { 2 } else { 1 },
        &reply_results,
    )
    .await?;
    tx.commit().await?;

    changed.item_ids.extend(consumed.item_ids.iter().copied());
    changed
        .currency_ids
        .extend(consumed.currency_ids.iter().copied());

    Ok(SummonCompletion {
        reply: SummonReply {
            summon_result: reply_results,
        },
        changed,
        guide_info,
    })
}

pub struct SummonCompletion {
    pub reply: SummonReply,
    pub changed: reward::AppliedRewards,
    pub guide_info: Option<GuideInfo>,
}

pub(super) fn is_newbie_pool(pool: &config::summon_pool::SummonPool) -> bool {
    SummonType::from(pool.r#type) == SummonType::Newbie
}

fn newbie_pull_limit(pool: &config::summon_pool::SummonPool) -> Result<i32, AppError> {
    pool.award_time
        .split('|')
        .nth(1)
        .and_then(|limit| limit.parse().ok())
        .filter(|limit| *limit > 0)
        .ok_or(AppError::InvalidRequest)
}

pub(super) fn validate_summon_count(count: i32) -> Result<(), AppError> {
    matches!(count, 1 | 10)
        .then_some(())
        .ok_or(AppError::InvalidRequest)
}

pub(super) async fn select_summon_cost(
    db: &SqlitePool,
    player_id: i64,
    cost: String,
) -> Result<reward::RewardSet, AppError> {
    let cost_options = cost.split('|').map(reward::parse).collect::<Vec<_>>();
    for option in &cost_options {
        if can_pay(db, player_id, option).await? {
            return Ok(option.clone());
        }
    }

    let Some(mut fallback) = cost_options.last().cloned() else {
        return Ok(reward::RewardSet::default());
    };
    if fallback.items.len() != 1 || !fallback.currencies.is_empty() {
        return Err(AppError::InsufficientCurrency);
    }
    let (item_id, required) = fallback.items[0];
    let owned = UserItemModel::new(player_id, db.clone())
        .get_item(item_id)
        .await?
        .map(|item| item.quantity)
        .unwrap_or_default()
        .clamp(0, required);
    let missing = required.saturating_sub(owned);
    let currency_cost = config::configs::get()
        .r#const
        .get(SummonConstId::SinglePullCurrencyCost as i32)
        .map(|row| reward::parse(&row.value))
        .and_then(|cost| cost.currencies.into_iter().next())
        .filter(|(_, amount)| *amount > 0)
        .ok_or(AppError::InvalidRequest)?;
    fallback.items = (owned > 0)
        .then_some((item_id, owned))
        .into_iter()
        .collect();
    fallback
        .currencies
        .push((currency_cost.0, currency_cost.1.saturating_mul(missing)));
    if can_pay(db, player_id, &fallback).await? {
        return Ok(fallback);
    }

    Err(AppError::InsufficientCurrency)
}

async fn can_pay(
    db: &SqlitePool,
    player_id: i64,
    rewards: &reward::RewardSet,
) -> Result<bool, AppError> {
    let items = UserItemModel::new(player_id, db.clone());
    for (item_id, amount) in &rewards.items {
        let current = items
            .get_item(*item_id)
            .await?
            .map(|item| item.quantity)
            .unwrap_or_default();
        if current < *amount {
            return Ok(false);
        }
    }

    let currencies = UserCurrencyModel::new(player_id, db.clone());
    for (currency_id, amount) in &rewards.currencies {
        let current = currencies
            .get_currency(*currency_id)
            .await?
            .map(|currency| currency.quantity)
            .unwrap_or_default();
        if current < *amount {
            return Ok(false);
        }
    }

    Ok(!rewards.items.is_empty() || !rewards.currencies.is_empty())
}
