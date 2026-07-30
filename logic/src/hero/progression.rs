use super::*;

impl HeroManager {
    pub async fn level_up(
        self,
        db: &SqlitePool,
        hero_id: i32,
        new_level: i32,
    ) -> Result<(HeroLevelUpReply, HeroInfo, ConsumedRewards), AppError> {
        let tables = config::configs::get();
        let hero = UserHeroModel::new(self.player_id, db.clone());
        let current = hero.get(hero_id).await?;
        let max_level = tables
            .character_rank_level_limit(hero_id, current.record.rank)
            .ok_or(AppError::InvalidRequest)?;
        if new_level <= current.record.level || new_level > max_level {
            return Err(AppError::InvalidRequest);
        }
        let rare = tables
            .character
            .get(hero_id)
            .map(|hero| hero.rare)
            .ok_or(AppError::InvalidRequest)?;
        let mut costs = reward::RewardSet::default();
        for level in current.record.level + 1..=new_level {
            let row = tables
                .character_level_cost(rare, level)
                .ok_or(AppError::InvalidRequest)?;
            costs.extend(reward::parse(&row.cosume));
        }

        let mut tx = db.begin().await?;
        let consumed = reward::consume(&mut tx, self.player_id, &costs).await?;
        if !hero
            .level_up(&mut tx, hero_id, current.record.level, new_level)
            .await?
        {
            return Err(AppError::InvalidRequest);
        }
        tx.commit().await?;
        let updated = snapshot(db, hero.get(hero_id).await?).await?;

        Ok((
            HeroLevelUpReply {
                hero_id: Some(hero_id),
                new_level: Some(new_level),
            },
            updated,
            consumed,
        ))
    }

    pub async fn rank_up(
        self,
        db: &SqlitePool,
        hero_id: i32,
    ) -> Result<(HeroRankUpReply, HeroInfo, ConsumedRewards), AppError> {
        let tables = config::configs::get();
        let hero = UserHeroModel::new(self.player_id, db.clone());
        let current = hero.get(hero_id).await?;
        let current_rank = current.record.rank;
        let new_rank = current_rank
            .checked_add(1)
            .ok_or(AppError::InvalidRequest)?;
        let rank = tables
            .character_rank(hero_id, new_rank)
            .ok_or(AppError::InvalidRequest)?;
        if required_rank_level(&rank.requirement) != Some(current.record.level) {
            return Err(AppError::InvalidRequest);
        }

        let mut tx = db.begin().await?;
        let consumed =
            reward::consume(&mut tx, self.player_id, &reward::parse(&rank.consume)).await?;
        if !hero
            .rank_up_with_insight_skin_in_transaction(&mut tx, hero_id, current_rank)
            .await?
        {
            return Err(AppError::InvalidRequest);
        }
        tx.commit().await?;
        let updated = snapshot(db, hero.get(hero_id).await?).await?;

        Ok((
            HeroRankUpReply {
                hero_id: Some(hero_id),
                new_rank: Some(new_rank),
            },
            updated,
            consumed,
        ))
    }

    pub async fn upgrade_skill(
        self,
        db: &SqlitePool,
        hero_id: i32,
        skill_type: i32,
        levels: i32,
    ) -> Result<(HeroUpgradeSkillReply, HeroInfo, u32), AppError> {
        if skill_type != 3 {
            return Err(AppError::InvalidRequest);
        }

        let hero = UserHeroModel::new(self.player_id, db.clone());
        let current = hero.get(hero_id).await?;
        if current.record.ex_skill_level >= 5 {
            return Err(AppError::InvalidRequest);
        }

        let consume = levels.max(1).min(5 - current.record.ex_skill_level);
        let consumed_item_id = duplicate_item_id(hero_id)?;
        let mut tx = db.begin().await?;
        reward::consume(
            &mut tx,
            self.player_id,
            &reward::RewardSet {
                items: vec![(consumed_item_id, consume)],
                ..Default::default()
            },
        )
        .await?;
        if !hero
            .upgrade_ex_skill_in_transaction(
                &mut tx,
                hero_id,
                current.record.ex_skill_level,
                consume,
            )
            .await?
        {
            return Err(AppError::InvalidRequest);
        }
        tx.commit().await?;
        let updated = snapshot(db, hero.get(hero_id).await?).await?;

        Ok((HeroUpgradeSkillReply {}, updated, consumed_item_id))
    }
}

pub(super) fn required_rank_level(requirement: &str) -> Option<i32> {
    requirement.strip_prefix("1#")?.parse().ok()
}

pub(super) fn duplicate_item_id(hero_id: i32) -> Result<u32, AppError> {
    let character = config::configs::get()
        .character
        .get(hero_id)
        .ok_or(AppError::InvalidRequest)?;

    reward::parse(&character.duplicate_item)
        .items
        .first()
        .map(|(id, _)| *id)
        .ok_or(AppError::InvalidRequest)
}
