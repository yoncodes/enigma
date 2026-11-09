use super::*;

impl UserHeroModel {
    pub async fn get_hero(&self, hero_id: i32) -> Result<HeroData> {
        HeroModel::<HeroData>::get(self, hero_id).await
    }

    pub async fn get_all_heroes(&self) -> Result<Vec<HeroData>> {
        HeroModel::<HeroData>::get_all(self).await
    }

    pub async fn get_hero_create_times(&self) -> Result<Vec<(i32, i64)>> {
        Ok(sqlx::query_as(
            "SELECT hero_id, create_time FROM heroes WHERE user_id = ? ORDER BY hero_id",
        )
        .bind(self.user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn has_hero(&self, hero_id: i32) -> Result<bool> {
        HeroModel::<HeroData>::has_hero(self, hero_id).await
    }

    pub async fn equipped_skin(&self, hero_id: i32) -> Result<Option<i32>> {
        Ok(
            sqlx::query_scalar("SELECT skin FROM heroes WHERE user_id = ? AND hero_id = ?")
                .bind(self.user_id)
                .bind(hero_id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn player_hero_count(&self, rarity: usize, now: i64) -> Result<()> {
        HeroModel::<HeroData>::hero_count(self, rarity, now).await
    }

    pub async fn add_hero_duplicate(&self, hero_id: i32) -> Result<i32> {
        HeroModel::<HeroData>::hero_duplicate(self, hero_id).await
    }

    pub async fn create_hero(&self, hero_id: i32) -> Result<i64> {
        let mut tx = self.pool.begin().await?;
        let uid = HeroModel::<HeroData>::create_hero(self, &mut tx, hero_id).await?;
        tx.commit().await?;
        Ok(uid)
    }

    pub async fn create_hero_in_transaction(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        hero_id: i32,
    ) -> Result<i64> {
        HeroModel::<HeroData>::create_hero(self, tx, hero_id).await
    }

    pub async fn grant_hero_in_transaction(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        hero_id: i32,
    ) -> Result<HeroGrant> {
        let duplicate_count: Option<i32> = sqlx::query_scalar(
            "UPDATE heroes
             SET duplicate_count = duplicate_count + 1
             WHERE user_id = ? AND hero_id = ?
             RETURNING duplicate_count",
        )
        .bind(self.user_id)
        .bind(hero_id)
        .fetch_optional(&mut **tx)
        .await?;
        if let Some(duplicate_count) = duplicate_count {
            return Ok(HeroGrant {
                is_new: false,
                duplicate_count,
                initial_voice_ids: Vec::new(),
            });
        }

        self.create_hero_in_transaction(tx, hero_id).await?;
        Ok(HeroGrant {
            is_new: true,
            duplicate_count: 0,
            initial_voice_ids: initial_voice_ids(config::configs::get(), hero_id),
        })
    }

    pub async fn hero_uid_in_transaction(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        hero_id: i32,
    ) -> Result<i64> {
        Ok(
            sqlx::query_scalar("SELECT uid FROM heroes WHERE user_id = ? AND hero_id = ?")
                .bind(self.user_id)
                .bind(hero_id)
                .fetch_one(&mut **tx)
                .await?,
        )
    }

    pub async fn update_special_equipped_gear(
        &self,
        hero_id: i32,
        extra_str: String,
    ) -> Result<()> {
        HeroModel::<HeroData>::special_equipped_gear(self, hero_id, extra_str).await
    }

    pub async fn update_equipped_gear(&self, hero_id: i32, equip_uid: i64) -> Result<bool> {
        HeroModel::<HeroData>::equipped_gear(self, hero_id, equip_uid).await
    }

    pub async fn unmark_new(&self, hero_id: i32) -> Result<()> {
        HeroModel::<HeroData>::unmark_new(self, hero_id).await
    }
}
