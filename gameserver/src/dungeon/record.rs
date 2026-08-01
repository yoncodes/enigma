use super::*;

pub async fn load_dungeon_record(
    db: &SqlitePool,
    player_id: i64,
    episode_id: i32,
) -> Result<Option<sonettobuf::FightGroupRecord>, AppError> {
    Ok(dungeons::load_dungeon_record(db, player_id, episode_id).await?)
}

pub async fn load_dungeon_record_operations(
    db: &SqlitePool,
    player_id: i64,
    episode_id: i32,
) -> Result<Vec<sonettobuf::FightRoundOperRecord>, AppError> {
    Ok(dungeons::load_dungeon_record_operations(db, player_id, episode_id).await?)
}

pub async fn prepare_dungeon_record(
    db: &SqlitePool,
    player_id: i64,
    active: &ActiveBattle,
    round: i32,
) -> Result<DungeonRecordStatus, AppError> {
    let Some(episode) = configs::get().episode.get(active.episode_id) else {
        return Ok(DungeonRecordStatus::default());
    };
    let Some(fight_group) = active.fight_group.clone() else {
        return Ok(DungeonRecordStatus::default());
    };
    if episode.can_use_record == 0
        || active.is_replay.unwrap_or(false)
        || (episode.first_battle_id != 0 && active.battle_id == episode.first_battle_id)
    {
        return Ok(DungeonRecordStatus::default());
    }

    let old_round = dungeons::dungeon_record_round(db, player_id, active.episode_id).await?;
    let prepared = dungeons::prepare_dungeon_record(
        db,
        player_id,
        active.runtime.fight_version(),
        active.seed,
        &fight_group,
        &active.oper_records(),
    )
    .await?;
    let pending = PendingDungeonRecord {
        episode_id: active.episode_id,
        round,
        record: prepared,
    };

    if should_save_record(old_round, round) {
        return Ok(DungeonRecordStatus {
            old_round: old_round.unwrap_or_default(),
            new_round: round,
            auto_save: Some(pending),
            ..Default::default()
        });
    }

    Ok(DungeonRecordStatus {
        can_update: true,
        old_round: old_round.unwrap_or_default(),
        new_round: round,
        pending: Some(pending),
        ..Default::default()
    })
}

pub(super) fn should_save_record(old_round: Option<i32>, new_round: i32) -> bool {
    old_round.is_none_or(|old| new_round <= old)
}

pub async fn cover_dungeon_record(
    db: &SqlitePool,
    player_id: i64,
    pending: Option<PendingDungeonRecord>,
    cover: bool,
) -> Result<bool, AppError> {
    let Some(pending) = pending.filter(|_| cover) else {
        return Ok(false);
    };
    replace_dungeon_record(db, player_id, &pending).await?;
    Ok(true)
}

pub(super) async fn replace_dungeon_record(
    db: &SqlitePool,
    player_id: i64,
    record: &PendingDungeonRecord,
) -> Result<(), AppError> {
    let mut tx = db.begin().await?;
    dungeons::replace_prepared_dungeon_record_in_transaction(
        &mut tx,
        player_id,
        record.episode_id,
        record.round,
        &record.record,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub(super) async fn save_dungeon_record_if_faster_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    player_id: i64,
    record: &PendingDungeonRecord,
) -> Result<bool, AppError> {
    Ok(
        dungeons::save_prepared_dungeon_record_if_faster_in_transaction(
            tx,
            player_id,
            record.episode_id,
            record.round,
            &record.record,
        )
        .await?,
    )
}
