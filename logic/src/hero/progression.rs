use super::*;

impl HeroManager {
    pub async fn upgrade_materials(
        self,
        db: &SqlitePool,
        target_level: i32,
        target_talent: i32,
    ) -> Result<reward::RewardSet, AppError> {
        let tables = config::configs::get();
        if !(1..=tables.max_character_level()).contains(&target_level)
            || !(1..=15).contains(&target_talent)
        {
            return Err(AppError::InvalidRequest);
        }
        let heroes = UserHeroModel::new(self.player_id, db.clone())
            .get_all_heroes()
            .await?;
        let mut items = BTreeMap::<u32, i32>::new();
        let mut currencies = BTreeMap::<i32, i32>::new();

        for hero in heroes {
            let hero_id = hero.record.hero_id;
            let rare = tables
                .character
                .get(hero_id)
                .map(|row| row.rare)
                .ok_or_else(|| {
                    AppError::Custom(format!("hero {hero_id} has no character config"))
                })?;
            let max_level = tables
                .character_rank
                .iter()
                .filter(|row| row.hero_id == hero_id)
                .filter_map(|row| tables.character_rank_level_limit(hero_id, row.rank))
                .max()
                .ok_or_else(|| {
                    AppError::Custom(format!("hero {hero_id} has no character_rank config"))
                })?;
            let target_level = target_level.min(max_level);
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

            let max_talent = tables
                .character_talent
                .iter()
                .filter(|row| row.hero_id == hero_id)
                .map(|row| row.talent_id)
                .max()
                .ok_or_else(|| {
                    AppError::Custom(format!("hero {hero_id} has no character_talent config"))
                })?;
            for talent in hero.record.talent + 1..=target_talent.min(max_talent) {
                let row = tables.character_talent(hero_id, talent).ok_or_else(|| {
                    AppError::Custom(format!("hero {hero_id} has no talent {talent} config"))
                })?;
                add_material_costs(reward::parse(&row.consume), &mut items, &mut currencies);
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
