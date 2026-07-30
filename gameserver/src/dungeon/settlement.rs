use super::*;

#[derive(Debug, Clone, Default)]
pub struct DungeonRecordStatus {
    pub can_update: bool,
    pub old_round: i32,
    pub new_round: i32,
    pub pending: Option<PendingDungeonRecord>,
    pub(super) auto_save: Option<PendingDungeonRecord>,
}

pub struct DungeonCompletion<'a> {
    pub star: i32,
    pub total_round: i32,
    pub multiplier: i32,
    pub fight_group: Option<&'a sonettobuf::FightGroup>,
}

pub struct DungeonSettlement {
    pub hero_ids: Vec<i32>,
    pub rewards: AppliedRewards,
    pub dungeon_update: DungeonUpdatePush,
    pub open_infos: Vec<sonettobuf::OpenInfo>,
    pub end_dungeon: EndDungeonPush,
    pub compose_push: Option<sonettobuf::TowerComposeFightSettlePush>,
}

pub struct BattlelessSettlement {
    pub cost: ConsumedRewards,
    pub dungeon: DungeonSettlement,
}

pub struct RefundSettlement {
    pub rewards: AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
    pub compose_push: Option<sonettobuf::TowerComposeFightSettlePush>,
}

pub async fn finish_fight_instance(
    db: &SqlitePool,
    player_id: i64,
    fight_id: i64,
) -> Result<(), AppError> {
    battle_db::finish_fight_instance(db, player_id, fight_id).await?;
    Ok(())
}

pub async fn settle_active(
    db: &SqlitePool,
    player_id: i64,
    active: &ActiveBattle,
    completion: DungeonCompletion<'_>,
    record: &DungeonRecordStatus,
) -> Result<DungeonSettlement, AppError> {
    let fight_id = active.fight_id.ok_or(AppError::InvalidRequest)?;
    let mut tx = db.begin().await?;
    let mut settlement = settle_completion_in_transaction(
        &mut tx,
        player_id,
        active.chapter_id,
        active.episode_id,
        completion,
        record,
    )
    .await?;
    settlement.compose_push =
        tower_compose::settle_in_transaction(&mut tx, player_id, active).await?;
    battle_db::finish_fight_instance_in_transaction(&mut tx, player_id, fight_id).await?;
    tx.commit().await?;
    Ok(settlement)
}

pub async fn settle_battleless(
    db: &SqlitePool,
    player_id: i64,
    chapter_id: i32,
    episode_id: i32,
    completion: DungeonCompletion<'_>,
    record: &DungeonRecordStatus,
) -> Result<BattlelessSettlement, AppError> {
    let episode = configs::get()
        .episode
        .get(episode_id)
        .ok_or(AppError::InvalidRequest)?;
    if episode.battle_id != 0 {
        return Err(AppError::InvalidRequest);
    }
    let mut tx = db.begin().await?;
    if dungeons::episode_star_in_transaction(&mut tx, player_id, episode_id).await? > 0 {
        return Err(AppError::InvalidRequest);
    }
    let cost = reward::RewardManager::new(player_id)
        .consume(&mut tx, &episode_cost(episode, completion.multiplier))
        .await?;
    let dungeon = settle_completion_in_transaction(
        &mut tx, player_id, chapter_id, episode_id, completion, record,
    )
    .await?;
    tx.commit().await?;
    Ok(BattlelessSettlement { cost, dungeon })
}

async fn settle_completion_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    player_id: i64,
    chapter_id: i32,
    episode_id: i32,
    completion: DungeonCompletion<'_>,
    record: &DungeonRecordStatus,
) -> Result<DungeonSettlement, AppError> {
    let DungeonCompletion {
        star,
        total_round,
        multiplier,
        fight_group,
    } = completion;
    let episode = configs::get()
        .episode
        .get(episode_id)
        .ok_or(AppError::InvalidRequest)?;
    let cost = episode_cost_value(episode);

    let previous_star = dungeons::episode_star_in_transaction(tx, player_id, episode_id).await?;
    let first_pass = previous_star == 0;
    let (mut dungeon_info, chapter_type_nums) = dungeons::update_dungeon_progress_in_transaction(
        tx, player_id, chapter_id, episode_id, star,
    )
    .await?;
    let record_updated = if let Some(record) = &record.auto_save {
        let updated = save_dungeon_record_if_faster_in_transaction(tx, player_id, record).await?;
        dungeon_info.has_record = true;
        updated
    } else {
        false
    };
    let hero_ids = if let Some(fight_group) = fight_group {
        HeroManager::new(player_id)
            .gain_battle_faith_in_transaction(tx, fight_group, cost.saturating_mul(multiplier))
            .await?
    } else {
        Vec::new()
    };
    let completion_rewards =
        logic::dungeon::completion_rewards(episode, first_pass, previous_star, star, multiplier);
    let rewards = reward::RewardManager::new(player_id)
        .apply_dungeon_in_transaction(tx, completion_rewards.rewards)
        .await?;
    let open_infos = open_infos::reconcile_progression_in_transaction(tx, player_id).await?;

    Ok(DungeonSettlement {
        hero_ids,
        rewards,
        dungeon_update: DungeonUpdatePush {
            dungeon_info: Some(dungeon_info.into()),
            chapter_type_nums: chapter_type_nums.into_iter().map(Into::into).collect(),
        },
        open_infos,
        end_dungeon: EndDungeonPush {
            chapter_id: Some(chapter_id),
            episode_id: Some(episode_id),
            player_exp: Some(completion_rewards.player_exp),
            first_bonus: material_data(completion_rewards.first_bonus),
            normal_bonus: material_data(completion_rewards.normal_bonus),
            star: Some(star),
            advenced_bonus: material_data(completion_rewards.advanced_bonus),
            update_dungeon_record: Some(record_updated),
            can_update_dungeon_record: Some(record.can_update),
            old_record_round: Some(record.old_round),
            new_record_round: Some(record.new_round),
            first_pass: Some(first_pass),
            addition_bonus: Vec::new(),
            time_first_bonus: Vec::new(),
            extra_str: Some(String::new()),
            drop_bonus: Vec::new(),
            assist_user_id: Some(0),
            assist_nickname: Some(String::new()),
            total_round: Some(total_round),
        },
        compose_push: None,
    })
}

pub async fn settle_refund(
    db: &SqlitePool,
    player_id: i64,
    active: &ActiveBattle,
    include_compose: bool,
) -> Result<RefundSettlement, AppError> {
    let episode = configs::get()
        .episode
        .get(active.episode_id)
        .ok_or(AppError::InvalidRequest)?;
    let refund = failure_refund(episode, active.multiplication.unwrap_or(1).max(1));
    let fight_id = active.fight_id.ok_or(AppError::InvalidRequest)?;
    settle_refund_rewards(
        db,
        player_id,
        fight_id,
        refund,
        include_compose.then_some(active),
    )
    .await
}

pub async fn settle_checkpoint_refund(
    db: &SqlitePool,
    player_id: i64,
    fight_id: i64,
    refund: reward::RewardSet,
) -> Result<RefundSettlement, AppError> {
    settle_refund_rewards(db, player_id, fight_id, refund, None).await
}

async fn settle_refund_rewards(
    db: &SqlitePool,
    player_id: i64,
    fight_id: i64,
    refund: reward::RewardSet,
    compose_active: Option<&ActiveBattle>,
) -> Result<RefundSettlement, AppError> {
    let material_changes = refund.material_changes();
    let mut tx = db.begin().await?;
    let rewards = reward::RewardManager::new(player_id)
        .apply_dungeon_in_transaction(&mut tx, refund)
        .await?;
    let compose_push = match compose_active {
        Some(active) => tower_compose::settle_in_transaction(&mut tx, player_id, active).await?,
        None => None,
    };
    battle_db::finish_fight_instance_in_transaction(&mut tx, player_id, fight_id).await?;
    tx.commit().await?;
    Ok(RefundSettlement {
        rewards,
        material_changes,
        compose_push,
    })
}

fn material_data(changes: Vec<(u32, u32, i32)>) -> Vec<MaterialData> {
    changes
        .into_iter()
        .map(|(materil_type, materil_id, quantity)| MaterialData {
            materil_type: Some(materil_type),
            materil_id: Some(materil_id),
            quantity: Some(quantity),
        })
        .collect()
}
