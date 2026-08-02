use super::*;

impl HeroManager {
    pub async fn destiny_stone(
        self,
        db: &SqlitePool,
        hero_id: i32,
        stone_id: i32,
    ) -> Result<(DestinyStoneUseReply, HeroInfo), AppError> {
        let hero = UserHeroModel::new(self.player_id, db.clone());
        let current = hero.get(hero_id).await?;
        if stone_id != 0
            && (!destiny_stones(hero_id).contains(&stone_id)
                || !current.destiny_stone_unlocks.contains(&stone_id))
        {
            return Err(AppError::InvalidRequest);
        }
        hero.update_destiny_stone(hero_id, stone_id).await?;
        let updated = snapshot(db, hero.get(hero_id).await?).await?;

        Ok((
            DestinyStoneUseReply {
                hero_id: Some(hero_id),
                stone_id: Some(stone_id),
            },
            updated,
        ))
    }

    pub async fn destiny_rank_up(
        self,
        db: &SqlitePool,
        hero_id: i32,
    ) -> Result<(DestinyRankUpReply, HeroInfo, ConsumedRewards), AppError> {
        let hero = UserHeroModel::new(self.player_id, db.clone());
        let current = hero.get(hero_id).await?;
        if !destiny_available(hero_id, current.record.rank, current.record.level) {
            return Err(AppError::InvalidRequest);
        }
        let slot = next_destiny_slot(
            hero_id,
            current.record.destiny_rank,
            current.record.destiny_level,
        )
        .filter(|slot| {
            slot.node == 1 && slot.stage == current.record.destiny_rank.saturating_add(1)
        })
        .ok_or(AppError::InvalidRequest)?;
        let mut tx = db.begin().await?;
        let consumed =
            reward::consume(&mut tx, self.player_id, &reward::parse(&slot.consume)).await?;
        if !hero
            .update_destiny_progress_in_transaction(
                &mut tx,
                hero_id,
                current.record.destiny_rank,
                current.record.destiny_level,
                slot.stage,
                slot.node,
            )
            .await?
        {
            return Err(AppError::InvalidRequest);
        }
        tx.commit().await?;
        let updated = snapshot(db, hero.get(hero_id).await?).await?;

        Ok((
            DestinyRankUpReply {
                hero_id: Some(hero_id),
            },
            updated,
            consumed,
        ))
    }

    pub async fn destiny_level_up(
        self,
        db: &SqlitePool,
        hero_id: i32,
        level: i32,
    ) -> Result<(DestinyLevelUpReply, HeroInfo, ConsumedRewards), AppError> {
        let hero = UserHeroModel::new(self.player_id, db.clone());
        let current = hero.get(hero_id).await?;
        if current.record.destiny_rank <= 0 || level <= current.record.destiny_level {
            return Err(AppError::InvalidRequest);
        }

        let tables = config::configs::get();
        let destiny = tables
            .character_destiny(hero_id)
            .ok_or(AppError::InvalidRequest)?;
        let slots = (current.record.destiny_level + 1..=level)
            .map(|node| {
                tables
                    .character_destiny_slot(destiny.slots_id, current.record.destiny_rank, node)
                    .ok_or(AppError::InvalidRequest)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let costs = slots
            .iter()
            .map(|slot| slot.consume.as_str())
            .filter(|cost| !cost.is_empty())
            .collect::<Vec<_>>()
            .join("|");
        let mut tx = db.begin().await?;
        let consumed = reward::consume(&mut tx, self.player_id, &reward::parse(&costs)).await?;
        if !hero
            .update_destiny_progress_in_transaction(
                &mut tx,
                hero_id,
                current.record.destiny_rank,
                current.record.destiny_level,
                current.record.destiny_rank,
                level,
            )
            .await?
        {
            return Err(AppError::InvalidRequest);
        }
        tx.commit().await?;
        let updated = snapshot(db, hero.get(hero_id).await?).await?;

        Ok((
            DestinyLevelUpReply {
                hero_id: Some(hero_id),
                level: Some(level),
            },
            updated,
            consumed,
        ))
    }

    pub async fn destiny_stone_unlock(
        self,
        db: &SqlitePool,
        hero_id: i32,
        stone_id: i32,
    ) -> Result<(DestinyStoneUnlockReply, HeroInfo, ConsumedRewards), AppError> {
        let hero = UserHeroModel::new(self.player_id, db.clone());
        let current = hero.get(hero_id).await?;
        if !destiny_available(hero_id, current.record.rank, current.record.level)
            || current.record.destiny_rank <= 0
            || !destiny_stones(hero_id).contains(&stone_id)
            || current.destiny_stone_unlocks.contains(&stone_id)
        {
            return Err(AppError::InvalidRequest);
        }
        let config = config::configs::get()
            .character_destiny_stone_cost(stone_id)
            .ok_or(AppError::InvalidRequest)?;
        let mut tx = db.begin().await?;
        let consumed =
            reward::consume(&mut tx, self.player_id, &reward::parse(&config.consume)).await?;
        if !hero
            .unlock_destiny_stone_in_transaction(&mut tx, hero_id, stone_id)
            .await?
        {
            return Err(AppError::InvalidRequest);
        }
        tx.commit().await?;
        let updated = snapshot(db, hero.get(hero_id).await?).await?;

        Ok((
            DestinyStoneUnlockReply {
                hero_id: Some(hero_id),
                stone_id: Some(stone_id),
            },
            updated,
            consumed,
        ))
    }
}

pub fn destiny_stones(hero_id: i32) -> Vec<i32> {
    config::configs::get()
        .character_destiny(hero_id)
        .map(|row| {
            row.facets_id
                .split('#')
                .filter_map(|id| id.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

pub fn destiny_available(hero_id: i32, rank: i32, level: i32) -> bool {
    let tables = config::configs::get();
    let Some(character) = tables.character.get(hero_id) else {
        return false;
    };
    let required_level = match character.rare {
        4 => 1,
        5 => 30,
        _ => return false,
    };
    let Some(level_offset) = tables.character_rank_level_limit(hero_id, 3) else {
        return false;
    };

    rank >= 4 && level >= level_offset + required_level
}

pub(super) fn next_destiny_slot(
    hero_id: i32,
    rank: i32,
    level: i32,
) -> Option<&'static config::character_destiny_slots::CharacterDestinySlots> {
    let tables = config::configs::get();
    let slots_id = tables.character_destiny(hero_id)?.slots_id;
    let find = |stage, node| tables.character_destiny_slot(slots_id, stage, node);

    if rank == 0 {
        find(1, 1)
    } else {
        find(rank, level + 1).or_else(|| find(rank + 1, 1))
    }
}
