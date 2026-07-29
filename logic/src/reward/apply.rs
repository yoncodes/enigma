use super::*;

async fn apply_bp_scores_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    player_id: i64,
    scores: Vec<(i32, i32)>,
) -> Result<Vec<BpScoreGain>, AppError> {
    let Some(current_bp_id) = task_db::current_battle_pass_id() else {
        return Ok(Vec::new());
    };

    let mut changed = Vec::new();
    for (bp_id, score) in scores {
        if bp_id != current_bp_id || score <= 0 {
            continue;
        }

        let update = battle_pass::add_score_in_transaction(tx, player_id, bp_id, score).await?;
        if update.score_changed {
            changed.push(BpScoreGain {
                bp_id,
                score: update.score,
                weekly_score: update.weekly_score,
            });
        }
    }

    Ok(changed)
}

pub(crate) async fn apply(
    db: &SqlitePool,
    player_id: i64,
    rewards: RewardSet,
) -> Result<AppliedRewards, AppError> {
    let mut tx = db.begin().await?;
    let applied = apply_in_transaction(&mut tx, db, player_id, rewards).await?;
    tx.commit().await?;
    Ok(applied)
}

pub(crate) async fn apply_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    db: &SqlitePool,
    player_id: i64,
    mut rewards: RewardSet,
) -> Result<AppliedRewards, AppError> {
    validate_grants(&rewards)?;
    let mut player_info_changed = false;
    while rewards.player_exp > 0 {
        let change = player_infos::add_exp_in_transaction(
            tx,
            player_id,
            std::mem::take(&mut rewards.player_exp),
        )
        .await?;
        player_info_changed = true;
        rewards.extend(profile::level_up_rewards(change));
        validate_grants(&rewards)?;
    }

    let heroes = UserHeroModel::new(player_id, db.clone());
    let mut hero_ids = Vec::new();
    let mut pending_heroes = std::collections::VecDeque::from(std::mem::take(&mut rewards.heroes));
    while let Some((hero_id, count)) = pending_heroes.pop_front() {
        for _ in 0..count {
            let grant = heroes.grant_hero_in_transaction(tx, hero_id).await?;
            hero_ids.push(hero_id);
            if !grant.is_new && grant.duplicate_count > 0 {
                let mut duplicate = hero_duplicate_rewards(hero_id, grant.duplicate_count)?;
                validate_grants(&duplicate)?;
                pending_heroes.extend(std::mem::take(&mut duplicate.heroes));
                rewards.extend(duplicate);
            }
        }
    }

    let now = common::time::ServerTime::now_ms();
    let mut item_ids = Vec::new();
    for (item_id, amount) in rewards.items {
        if amount == 0 {
            continue;
        }
        items::add_item_in_transaction(tx, player_id, item_id, amount, now).await?;
        item_ids.push(item_id);
    }

    let mut currency_ids = Vec::new();
    for (currency_id, amount) in rewards.currencies {
        if amount == 0 {
            continue;
        }
        currencies::add_currency_in_transaction(tx, player_id, currency_id, amount, now).await?;
        currency_ids.push((currency_id, amount));
    }

    rewards.equips.retain(|(_, count)| *count > 0);
    rewards.power_items.retain(|(_, count)| *count > 0);
    rewards.insight_items.retain(|(_, count)| *count > 0);
    let equip_uids =
        equipment::add_equipments_in_transaction(tx, player_id, &rewards.equips).await?;
    let power_item_ids =
        items::add_power_items_in_transaction(tx, player_id, &rewards.power_items).await?;
    let insight_item_ids =
        items::add_insight_items_in_transaction(tx, player_id, &rewards.insight_items).await?;
    let mut skin_gains = Vec::new();
    for (skin_id, count) in rewards.skins {
        if count > 0 {
            skin_gains.push(SkinGain {
                skin_id,
                first_gain: heroes.unlock_skin_in_transaction(tx, skin_id).await?,
            });
        }
    }
    let mut room_buildings = Vec::new();
    for (building_id, count) in rewards.room_buildings {
        for _ in 0..count {
            room_buildings
                .push(buildings::create_building_in_transaction(tx, player_id, building_id).await?);
        }
    }
    let mut cloth_updates = Vec::new();
    for (cloth_id, count) in rewards.player_cloths {
        if count > 0 {
            cloth_updates.push(cloths::unlock_in_transaction(tx, player_id, cloth_id).await?);
        }
    }
    for (cloth_id, amount) in rewards.player_cloth_exp {
        if amount > 0 {
            cloth_updates
                .push(cloths::add_exp_in_transaction(tx, player_id, cloth_id, amount).await?);
        }
    }
    let mut block_packages = Vec::new();
    for (package_id, count) in rewards.block_packages {
        if count > 0 {
            block_packages.push(
                block_packages::add_block_package_in_transaction(tx, player_id, package_id).await?,
            );
        }
    }
    let mut special_blocks = Vec::new();
    for (block_id, count) in rewards.special_blocks {
        let mut gained = None;
        for _ in 0..count {
            gained = Some(
                block_packages::add_special_block_in_transaction(tx, player_id, block_id).await?,
            );
        }
        if count > 0
            && let Some(gained) = gained
        {
            special_blocks.push(gained);
        }
    }
    let mut antiques = Vec::new();
    for (antique_id, count) in rewards.antiques {
        for _ in 0..count {
            antiques.push(
                antiques::add_antique_in_transaction(tx, player_id, antique_id)
                    .await?
                    .into(),
            );
        }
    }
    let bp_scores = apply_bp_scores_in_transaction(tx, player_id, rewards.bp_scores).await?;

    Ok(AppliedRewards {
        player_info_changed,
        item_ids,
        currency_ids,
        hero_ids,
        skin_gains,
        cloth_updates,
        equip_uids,
        power_item_ids,
        antiques,
        insight_item_ids,
        bp_scores,
        room_buildings,
        block_packages,
        special_blocks,
    })
}

fn validate_grants(rewards: &RewardSet) -> Result<(), AppError> {
    let valid = rewards.player_exp >= 0
        && [
            &rewards.currencies,
            &rewards.block_packages,
            &rewards.heroes,
            &rewards.skins,
            &rewards.player_cloths,
            &rewards.player_cloth_exp,
            &rewards.equips,
            &rewards.power_items,
            &rewards.room_buildings,
            &rewards.special_blocks,
            &rewards.antiques,
            &rewards.insight_items,
            &rewards.bp_scores,
        ]
        .into_iter()
        .all(|entries| entries.iter().all(|(_, count)| *count >= 0))
        && rewards.items.iter().all(|(_, count)| *count >= 0);
    valid
        .then_some(())
        .ok_or_else(|| AppError::Custom("reward grants cannot contain negative amounts".into()))
}

pub(crate) async fn apply_dungeon_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    player_id: i64,
    mut rewards: RewardSet,
) -> Result<AppliedRewards, AppError> {
    validate_grants(&rewards)?;
    ensure_dungeon_rewards_supported(&rewards)?;

    let mut player_info_changed = false;
    while rewards.player_exp > 0 {
        let change = player_infos::add_exp_in_transaction(
            tx,
            player_id,
            std::mem::take(&mut rewards.player_exp),
        )
        .await?;
        player_info_changed = true;
        rewards.extend(profile::level_up_rewards(change));
        validate_grants(&rewards)?;
        ensure_dungeon_rewards_supported(&rewards)?;
    }

    let now = common::time::ServerTime::now_ms();
    let mut item_ids = Vec::new();
    for (item_id, amount) in rewards.items {
        if amount == 0 {
            continue;
        }
        items::add_item_in_transaction(tx, player_id, item_id, amount, now).await?;
        item_ids.push(item_id);
    }

    let mut currency_ids = Vec::new();
    for (currency_id, amount) in rewards.currencies {
        if amount == 0 {
            continue;
        }
        currencies::add_currency_in_transaction(tx, player_id, currency_id, amount, now).await?;
        currency_ids.push((currency_id, amount));
    }

    let equip_uids =
        equipment::add_equipments_in_transaction(tx, player_id, &rewards.equips).await?;
    let mut cloth_updates = Vec::new();
    for (cloth_id, count) in rewards.player_cloths {
        if count > 0 {
            cloth_updates.push(cloths::unlock_in_transaction(tx, player_id, cloth_id).await?);
        }
    }
    let mut room_buildings = Vec::new();
    for (building_id, count) in rewards.room_buildings {
        for _ in 0..count {
            room_buildings
                .push(buildings::create_building_in_transaction(tx, player_id, building_id).await?);
        }
    }
    let mut block_packages = Vec::new();
    for (package_id, count) in rewards.block_packages {
        if count > 0 {
            block_packages.push(
                block_packages::add_block_package_in_transaction(tx, player_id, package_id).await?,
            );
        }
    }

    Ok(AppliedRewards {
        player_info_changed,
        item_ids,
        currency_ids,
        cloth_updates,
        equip_uids,
        room_buildings,
        block_packages,
        ..Default::default()
    })
}

fn ensure_dungeon_rewards_supported(rewards: &RewardSet) -> Result<(), AppError> {
    if rewards.heroes.is_empty()
        && rewards.skins.is_empty()
        && rewards.player_cloth_exp.is_empty()
        && rewards.power_items.is_empty()
        && rewards.special_blocks.is_empty()
        && rewards.antiques.is_empty()
        && rewards.insight_items.is_empty()
        && rewards.bp_scores.is_empty()
    {
        Ok(())
    } else {
        Err(AppError::Custom(
            "dungeon settlement contains an unsupported reward material".into(),
        ))
    }
}
