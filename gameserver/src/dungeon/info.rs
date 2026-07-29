use super::*;

pub struct DungeonMapProgression {
    pub map_ids: Vec<i32>,
    pub elements: Vec<i32>,
}

pub async fn reconcile_map_progression(
    db: &SqlitePool,
    player_id: i64,
) -> Result<DungeonMapProgression, AppError> {
    let (map_ids, elements) = dungeons::reconcile_map_progression(db, player_id).await?;
    Ok(DungeonMapProgression { map_ids, elements })
}

pub async fn reconcile_instruction_dungeon(
    db: &SqlitePool,
    player_id: i64,
) -> Result<Option<sonettobuf::InstructionDungeonInfoPush>, AppError> {
    if !instruction_dungeon::reconcile_unlocks(db, player_id).await? {
        return Ok(None);
    }
    Ok(Some(instruction_dungeon_push(db, player_id).await?))
}

pub async fn instruction_dungeon_push(
    db: &SqlitePool,
    player_id: i64,
) -> Result<sonettobuf::InstructionDungeonInfoPush, AppError> {
    let info = instruction_dungeon::get_info(db, player_id).await?;
    Ok(sonettobuf::InstructionDungeonInfoPush {
        unlock_ids: info.unlock_ids,
        get_reward_ids: info.get_reward_ids,
        get_final_reward: info.get_final_reward,
        open_ids: info.open_ids,
    })
}

pub async fn dungeon_info(
    db: &SqlitePool,
    player_id: i64,
) -> Result<
    (
        GetDungeonReply,
        Vec<database::models::game::dungeons::UserDungeon>,
    ),
    AppError,
> {
    dungeons::reconcile_map_progression(db, player_id).await?;
    let (
        dungeons,
        last_groups,
        maps,
        elements,
        reward_points,
        equip_sp,
        chapter_nums,
        finished_elements,
        finished_puzzles,
    ) = tokio::try_join!(
        dungeons::get_user_dungeons(db, player_id),
        dungeons::get_dungeon_last_hero_groups(db, player_id),
        dungeons::get_unlocked_maps(db, player_id),
        dungeons::get_elements(db, player_id),
        dungeons::get_reward_points(db, player_id),
        dungeons::get_equip_sp_chapters(db, player_id),
        dungeons::get_chapter_type_nums(db, player_id),
        dungeons::get_finished_elements(db, player_id),
        dungeons::get_finished_puzzles(db, player_id),
    )?;

    let dungeon_info_size = dungeons.len() as i32;
    Ok((
        GetDungeonReply {
            dungeon_info_list: Vec::new(),
            last_hero_group: last_groups.into_iter().map(Into::into).collect(),
            map_ids: maps,
            elements,
            reward_point_info: reward_points.into_iter().map(Into::into).collect(),
            equip_sp_chapters: equip_sp,
            chapter_type_nums: chapter_nums.into_iter().map(Into::into).collect(),
            finish_elements: finished_elements,
            finish_puzzles: finished_puzzles,
            dungeon_info_size: Some(dungeon_info_size),
        },
        dungeons,
    ))
}

pub async fn get_puzzle_progress(
    db: &SqlitePool,
    player_id: i64,
    element_id: i32,
) -> Result<GetPuzzleProgressReply, AppError> {
    if element_id <= 0 {
        return Err(AppError::InvalidRequest);
    }
    let progress = dungeons::get_puzzle_progress(db, player_id, element_id)
        .await?
        .ok_or(AppError::InvalidRequest)?;
    Ok(GetPuzzleProgressReply {
        element_id: Some(element_id),
        progress: Some(progress),
    })
}

pub async fn save_puzzle_progress(
    db: &SqlitePool,
    player_id: i64,
    element_id: i32,
    progress: String,
) -> Result<SavePuzzleProgressReply, AppError> {
    if element_id <= 0 || progress.len() > MAX_PUZZLE_PROGRESS_BYTES {
        return Err(AppError::InvalidRequest);
    }
    if !dungeons::save_puzzle_progress(db, player_id, element_id, &progress).await? {
        return Err(AppError::InvalidRequest);
    }
    Ok(SavePuzzleProgressReply {
        element_id: Some(element_id),
    })
}

pub async fn finish_puzzle(
    db: &SqlitePool,
    player_id: i64,
    element_id: i32,
) -> Result<PuzzleFinishReply, AppError> {
    if element_id <= 0 || !dungeons::finish_puzzle(db, player_id, element_id).await? {
        return Err(AppError::InvalidRequest);
    }
    Ok(PuzzleFinishReply {
        element_id: Some(element_id),
    })
}

pub async fn instruction_dungeon_info(
    db: &SqlitePool,
    player_id: i64,
) -> Result<InstructionDungeonInfoReply, AppError> {
    instruction_dungeon::reconcile_unlocks(db, player_id).await?;
    Ok(instruction_dungeon::get_info(db, player_id).await?)
}

pub async fn instruction_dungeon_open(
    db: &SqlitePool,
    player_id: i64,
    open_ids: Vec<i32>,
) -> Result<(InstructionDungeonOpenReply, bool), AppError> {
    let changed = instruction_dungeon::add_open_ids(db, player_id, open_ids).await?;
    Ok((InstructionDungeonOpenReply {}, changed))
}

pub async fn instruction_dungeon_reward(
    db: &SqlitePool,
    player_id: i64,
    topic_id: i32,
) -> Result<InstructionDungeonRewardClaim, AppError> {
    let topic = configs::get()
        .instruction_topic
        .get(topic_id)
        .ok_or(AppError::InvalidRequest)?;
    let mut tx = db.begin().await?;
    let rewards =
        if instruction_dungeon::claim_topic_reward_in_transaction(&mut tx, player_id, topic_id)
            .await?
        {
            reward::parse(&topic.bonus)
        } else {
            reward::RewardSet::default()
        };
    let material_changes = rewards.material_changes();
    let rewards = reward::RewardManager::new(player_id)
        .apply_in_transaction(&mut tx, db, rewards)
        .await?;
    tx.commit().await?;

    Ok(InstructionDungeonRewardClaim {
        reply: InstructionDungeonRewardReply {},
        rewards,
        material_changes,
    })
}

pub async fn instruction_dungeon_final_reward(
    db: &SqlitePool,
    player_id: i64,
) -> Result<InstructionDungeonFinalRewardClaim, AppError> {
    let mut tx = db.begin().await?;
    let rewards =
        if instruction_dungeon::claim_final_reward_in_transaction(&mut tx, player_id).await? {
            reward::parse(
                &configs::get()
                    .r#const
                    .get(TEACH_BOUNDS_CONFIG_ID)
                    .ok_or(AppError::InvalidRequest)?
                    .value,
            )
        } else {
            reward::RewardSet::default()
        };
    let material_changes = rewards.material_changes();
    let rewards = reward::RewardManager::new(player_id)
        .apply_in_transaction(&mut tx, db, rewards)
        .await?;
    tx.commit().await?;

    Ok(InstructionDungeonFinalRewardClaim {
        reply: InstructionDungeonFinalRewardReply {},
        rewards,
        material_changes,
    })
}
