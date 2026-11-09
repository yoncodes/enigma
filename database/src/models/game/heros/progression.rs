use super::*;

impl UserHeroModel {
    pub async fn update_destiny_stone(&self, hero_id: i32, stone_id: i32) -> Result<()> {
        HeroModel::<HeroData>::destiny_stone(self, hero_id, stone_id).await
    }

    pub async fn set_rank_and_level(&self, hero_id: i32, rank: i32, level: i32) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        self.set_rank_and_level_in_transaction(&mut tx, hero_id, rank, level)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn set_rank_and_level_in_transaction(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        hero_id: i32,
        rank: i32,
        level: i32,
    ) -> Result<()> {
        sqlx::query("UPDATE heroes SET rank = ?, level = ? WHERE user_id = ? AND hero_id = ?")
            .bind(rank)
            .bind(level)
            .bind(self.user_id)
            .bind(hero_id)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    pub async fn rank_up_with_insight_skin_in_transaction(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        hero_id: i32,
        current_rank: i32,
    ) -> Result<bool> {
        let new_rank = current_rank + 1;
        let updated = sqlx::query(
            "UPDATE heroes SET rank = ?, level = level + 1
             WHERE user_id = ? AND hero_id = ? AND rank = ?",
        )
        .bind(new_rank)
        .bind(self.user_id)
        .bind(hero_id)
        .bind(current_rank)
        .execute(&mut **tx)
        .await?;
        if updated.rows_affected() != 1 {
            return Ok(false);
        }

        if new_rank >= 3
            && let Some(skin_id) = config::configs::get()
                .skin
                .iter()
                .find(|skin| {
                    skin.character_id == hero_id && skin.id % 100 == 2 && skin.gain_approach == 1
                })
                .map(|skin| skin.id)
        {
            self.unlock_skin_in_transaction(tx, skin_id).await?;
        }

        Ok(true)
    }

    pub async fn apply_insight_item(&self, upgrade: InsightUpgrade) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        let consumed = sqlx::query(
            "UPDATE insight_items
             SET quantity = quantity - 1
             WHERE user_id = ? AND uid = ? AND item_id = ? AND quantity >= 1",
        )
        .bind(self.user_id)
        .bind(upgrade.item_uid)
        .bind(upgrade.item_id)
        .execute(&mut *tx)
        .await?;
        if consumed.rows_affected() != 1 {
            return Ok(false);
        }

        let hero_uid: Option<i64> = sqlx::query_scalar(
            "UPDATE heroes
             SET rank = ?, level = ?
             WHERE user_id = ? AND hero_id = ? AND rank = ? AND level = ?
             RETURNING uid",
        )
        .bind(upgrade.target_rank)
        .bind(upgrade.target_level)
        .bind(self.user_id)
        .bind(upgrade.hero_id)
        .bind(upgrade.current_rank)
        .bind(upgrade.current_level)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(_hero_uid) = hero_uid else {
            return Ok(false);
        };

        if upgrade.target_rank >= 3
            && let Some(skin_id) = config::configs::get()
                .skin
                .iter()
                .find(|skin| {
                    skin.character_id == upgrade.hero_id
                        && skin.id % 100 == 2
                        && skin.gain_approach == 1
                })
                .map(|skin| skin.id)
        {
            self.unlock_skin_in_transaction(&mut tx, skin_id).await?;
        }

        tx.commit().await?;
        Ok(true)
    }

    pub async fn upgrade_ex_skill_in_transaction(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        hero_id: i32,
        current_level: i32,
        levels: i32,
    ) -> Result<bool> {
        Ok(sqlx::query(
            "UPDATE heroes
             SET ex_skill_level = ex_skill_level + ?
             WHERE user_id = ? AND hero_id = ? AND ex_skill_level = ?
               AND ex_skill_level + ? <= 5",
        )
        .bind(levels)
        .bind(self.user_id)
        .bind(hero_id)
        .bind(current_level)
        .bind(levels)
        .execute(&mut **tx)
        .await?
        .rows_affected()
            == 1)
    }

    pub async fn update_destiny_progress_in_transaction(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        hero_id: i32,
        current_rank: i32,
        current_level: i32,
        rank: i32,
        level: i32,
    ) -> Result<bool> {
        Ok(sqlx::query(
            "UPDATE heroes
             SET destiny_rank = ?, destiny_level = ?
             WHERE user_id = ? AND hero_id = ?
               AND destiny_rank = ? AND destiny_level = ?",
        )
        .bind(rank)
        .bind(level)
        .bind(self.user_id)
        .bind(hero_id)
        .bind(current_rank)
        .bind(current_level)
        .execute(&mut **tx)
        .await?
        .rows_affected()
            == 1)
    }

    pub async fn unlock_destiny_stone_in_transaction(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        hero_id: i32,
        stone_id: i32,
    ) -> Result<bool> {
        let hero_uid: Option<i64> =
            sqlx::query_scalar("SELECT uid FROM heroes WHERE user_id = ? AND hero_id = ?")
                .bind(self.user_id)
                .bind(hero_id)
                .fetch_optional(&mut **tx)
                .await?;
        let Some(hero_uid) = hero_uid else {
            return Ok(false);
        };
        Ok(sqlx::query(
            "INSERT OR IGNORE INTO hero_destiny_stone_unlocks (hero_uid, stone_id)
             VALUES (?, ?)",
        )
        .bind(hero_uid)
        .bind(stone_id)
        .execute(&mut **tx)
        .await?
        .rows_affected()
            == 1)
    }

    pub async fn read_red_dot(&self, hero_id: i32, red_dot_type: i32) -> Result<i32> {
        HeroModel::<HeroData>::read_hero_red_dot(self, hero_id, red_dot_type).await
    }
}
