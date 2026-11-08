use super::*;

impl HeroManager {
    pub async fn upgrade_materials(
        self,
        db: &SqlitePool,
        hero_id: i32,
        target_level: i32,
        target_talent: i32,
        destiny_target: Option<DestinyMaterialTarget>,
    ) -> Result<reward::RewardSet, AppError> {
        let tables = config::configs::get();
        if !(1..=tables.max_character_level()).contains(&target_level)
            || !(1..=15).contains(&target_talent)
        {
            return Err(AppError::InvalidRequest);
        }
        let hero = UserHeroModel::new(self.player_id, db.clone())
            .get_hero(hero_id)
            .await
            .map_err(|_| AppError::InvalidRequest)?;
        if target_level < hero.record.level || target_talent < hero.record.talent {
            return Err(AppError::InvalidRequest);
        }
        let mut items = BTreeMap::<u32, i32>::new();
        let mut currencies = BTreeMap::<i32, i32>::new();

        let rare = tables
            .character
            .get(hero_id)
            .map(|row| row.rare)
            .ok_or_else(|| AppError::Custom(format!("hero {hero_id} has no character config")))?;
        let max_level = tables
            .character_rank
            .iter()
            .filter(|row| row.hero_id == hero_id)
            .filter_map(|row| tables.character_rank_level_limit(hero_id, row.rank))
            .max()
            .ok_or_else(|| {
                AppError::Custom(format!("hero {hero_id} has no character_rank config"))
            })?;
        let max_talent = tables
            .character_talent
            .iter()
            .filter(|row| row.hero_id == hero_id)
            .map(|row| row.talent_id)
            .max()
            .ok_or_else(|| {
                AppError::Custom(format!("hero {hero_id} has no character_talent config"))
            })?;
        if target_level > max_level || target_talent > max_talent {
            return Err(AppError::InvalidRequest);
        }
        let mut level = hero.record.level;

        for rank in hero.record.rank.. {
            if level >= target_level {
                break;
            }
            let level_limit = tables
                .character_rank_level_limit(hero_id, rank)
                .ok_or_else(|| {
                    AppError::Custom(format!("hero {hero_id} rank {rank} has no level limit"))
                })?;
            for next_level in level + 1..=target_level.min(level_limit) {
                let row = tables
                    .character_level_cost(rare, next_level)
                    .ok_or_else(|| {
                        AppError::Custom(format!(
                            "rarity {rare} level {next_level} has no character cost"
                        ))
                    })?;
                add_material_costs(reward::parse(&row.cosume), &mut items, &mut currencies);
            }
            level = target_level.min(level_limit);
            if level >= target_level {
                break;
            }

            let next_rank = tables.character_rank(hero_id, rank + 1).ok_or_else(|| {
                AppError::Custom(format!("hero {hero_id} has no rank {} config", rank + 1))
            })?;
            add_material_costs(
                reward::parse(&next_rank.consume),
                &mut items,
                &mut currencies,
            );
            level = level_limit.saturating_add(1);
        }

        for talent in hero.record.talent + 1..=target_talent {
            let row = tables.character_talent(hero_id, talent).ok_or_else(|| {
                AppError::Custom(format!("hero {hero_id} has no talent {talent} config"))
            })?;
            add_material_costs(reward::parse(&row.consume), &mut items, &mut currencies);
        }
        if let Some(destiny) = destiny_target {
            if !super::destiny::destiny_available(hero_id, hero.record.rank, hero.record.level)
                || !super::destiny::destiny_stones(hero_id).contains(&destiny.stone_id)
            {
                return Err(AppError::InvalidRequest);
            }
            let destiny_config = tables
                .character_destiny(hero_id)
                .ok_or(AppError::InvalidRequest)?;
            let max_rank = tables
                .character_destiny_slots
                .iter()
                .filter(|slot| slot.slots_id == destiny_config.slots_id)
                .map(|slot| slot.stage)
                .max()
                .ok_or(AppError::InvalidRequest)?;
            if !(hero.record.destiny_rank.max(1)..=max_rank).contains(&destiny.rank) {
                return Err(AppError::InvalidRequest);
            }
            for slot in tables
                .character_destiny_slots
                .iter()
                .filter(|slot| slot.slots_id == destiny_config.slots_id)
                .filter(|slot| slot.stage <= destiny.rank)
                .filter(|slot| {
                    slot.stage > hero.record.destiny_rank
                        || (slot.stage == hero.record.destiny_rank
                            && slot.node > hero.record.destiny_level)
                })
            {
                add_material_costs(reward::parse(&slot.consume), &mut items, &mut currencies);
            }
            if !hero.destiny_stone_unlocks.contains(&destiny.stone_id) {
                let cost = tables
                    .character_destiny_stone_cost(destiny.stone_id)
                    .ok_or(AppError::InvalidRequest)?;
                add_material_costs(reward::parse(&cost.consume), &mut items, &mut currencies);
            }
        }

        Ok(reward::RewardSet {
            items: items.into_iter().collect(),
            currencies: currencies.into_iter().collect(),
            ..Default::default()
        })
    }

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

    pub async fn set_level(
        self,
        db: &SqlitePool,
        hero_id: i32,
        target_level: i32,
    ) -> Result<LevelUpdate, AppError> {
        let tables = config::configs::get();
        let heroes = UserHeroModel::new(self.player_id, db.clone());
        let current = heroes
            .get_hero(hero_id)
            .await
            .map_err(|_| AppError::InvalidRequest)?;
        if target_level < current.record.level {
            return Err(AppError::InvalidRequest);
        }
        let target_rank = tables
            .character_rank
            .iter()
            .filter(|row| row.hero_id == hero_id)
            .filter_map(|row| {
                tables
                    .character_rank_level_limit(hero_id, row.rank)
                    .map(|limit| (row.rank, limit))
            })
            .filter(|(_, limit)| target_level <= *limit)
            .min_by_key(|(_, limit)| *limit)
            .map(|(rank, _)| rank)
            .ok_or(AppError::InvalidRequest)?;

        let skin_id = tables
            .skin
            .iter()
            .find(|skin| {
                target_rank >= 3
                    && skin.character_id == hero_id
                    && skin.id % 100 == 2
                    && skin.gain_approach == 1
            })
            .map(|skin| skin.id);
        let mut tx = db.begin().await?;
        heroes
            .set_rank_and_level_in_transaction(&mut tx, hero_id, target_rank, target_level)
            .await?;
        let skin_id = match skin_id {
            Some(skin_id) if heroes.unlock_skin_in_transaction(&mut tx, skin_id).await? => {
                Some(skin_id)
            }
            _ => None,
        };
        tx.commit().await?;

        Ok(LevelUpdate {
            hero: snapshot(db, heroes.get_hero(hero_id).await?).await?,
            skin_id,
        })
    }

    pub async fn set_portrait_level(
        self,
        db: &SqlitePool,
        hero_id: i32,
        target_level: i32,
    ) -> Result<PortraitUpdate, AppError> {
        if !(1..=5).contains(&target_level) {
            return Err(AppError::InvalidRequest);
        }
        let heroes = UserHeroModel::new(self.player_id, db.clone());
        let mut tx = db.begin().await?;
        let (item_id, granted) = self
            .set_portrait_level_in_transaction(&heroes, &mut tx, hero_id, target_level)
            .await?;
        tx.commit().await?;

        Ok(PortraitUpdate {
            hero: snapshot(db, heroes.get_hero(hero_id).await?).await?,
            item_id,
            granted,
        })
    }

    pub async fn set_resonance(
        self,
        db: &SqlitePool,
        hero_id: i32,
        target: i32,
    ) -> Result<HeroInfo, AppError> {
        let heroes = UserHeroModel::new(self.player_id, db.clone());
        let current = heroes
            .get_hero(hero_id)
            .await
            .map_err(|_| AppError::InvalidRequest)?;
        if target < current.record.talent {
            return Err(AppError::InvalidRequest);
        }
        let talent = config::configs::get()
            .character_talent(hero_id, target)
            .ok_or(AppError::InvalidRequest)?;
        if current.record.rank < talent.requirement {
            return Err(AppError::InvalidRequest);
        }
        if target != current.record.talent {
            let mut tx = db.begin().await?;
            if !heroes
                .update_talent(&mut tx, hero_id, current.record.talent, target)
                .await?
            {
                return Err(AppError::InvalidRequest);
            }
            tx.commit().await?;
        }
        snapshot(db, heroes.get_hero(hero_id).await?).await
    }

    pub async fn set_all_portrait_levels(
        self,
        db: &SqlitePool,
        target_level: i32,
    ) -> Result<PortraitBulkUpdate, AppError> {
        if !(1..=5).contains(&target_level) {
            return Err(AppError::InvalidRequest);
        }
        let heroes = UserHeroModel::new(self.player_id, db.clone());
        let mut tx = db.begin().await?;
        let hero_ids = sqlx::query_scalar::<_, i32>(
            "SELECT hero_id FROM heroes WHERE user_id = ? ORDER BY hero_id",
        )
        .bind(self.player_id)
        .fetch_all(&mut *tx)
        .await?;
        let mut item_ids = Vec::with_capacity(hero_ids.len());
        let mut granted = 0;
        for &hero_id in &hero_ids {
            let (item_id, added) = self
                .set_portrait_level_in_transaction(&heroes, &mut tx, hero_id, target_level)
                .await?;
            item_ids.push(item_id);
            granted += added;
        }
        tx.commit().await?;
        Ok(PortraitBulkUpdate {
            hero_ids,
            item_ids,
            granted,
        })
    }

    async fn set_portrait_level_in_transaction(
        self,
        heroes: &UserHeroModel,
        tx: &mut Transaction<'_, Sqlite>,
        hero_id: i32,
        target_level: i32,
    ) -> Result<(u32, i32), AppError> {
        let current = sqlx::query_scalar::<_, i32>(
            "SELECT ex_skill_level FROM heroes WHERE user_id = ? AND hero_id = ?",
        )
        .bind(self.player_id)
        .bind(hero_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::InvalidRequest)?;
        if target_level < current {
            return Err(AppError::InvalidRequest);
        }
        let item_id = duplicate_item_id(hero_id)?;
        let needed = target_level - current;
        if needed == 0 {
            return Ok((item_id, 0));
        }

        let owned = sqlx::query_scalar::<_, i32>(
            "SELECT COALESCE(quantity, 0) FROM items WHERE user_id = ? AND item_id = ?",
        )
        .bind(self.player_id)
        .bind(item_id)
        .fetch_optional(&mut **tx)
        .await?
        .unwrap_or_default();
        let granted = needed.saturating_sub(owned);
        if granted > 0 {
            items::add_item_quantity_in_transaction(tx, self.player_id, item_id, granted).await?;
        }
        reward::consume(
            tx,
            self.player_id,
            &reward::RewardSet {
                items: vec![(item_id, needed)],
                ..Default::default()
            },
        )
        .await?;
        if !heroes
            .upgrade_ex_skill_in_transaction(tx, hero_id, current, needed)
            .await?
        {
            return Err(AppError::InvalidRequest);
        }
        Ok((item_id, granted))
    }
}

fn add_material_costs(
    costs: reward::RewardSet,
    items: &mut BTreeMap<u32, i32>,
    currencies: &mut BTreeMap<i32, i32>,
) {
    for (id, amount) in costs.items {
        let total = items.entry(id).or_default();
        *total = total.saturating_add(amount);
    }
    for (id, amount) in costs.currencies {
        let total = currencies.entry(id).or_default();
        *total = total.saturating_add(amount);
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
