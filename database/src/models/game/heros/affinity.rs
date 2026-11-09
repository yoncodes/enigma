use super::*;

impl UserHeroModel {
    pub async fn get_touch_count(&self) -> Result<Option<i32>> {
        HeroModel::<HeroData>::touch_count(self).await
    }

    pub async fn use_touch(
        &self,
        hero_id: i32,
        faith_amount: i32,
        max_faith: i32,
    ) -> Result<Option<i32>> {
        HeroModel::<HeroData>::use_touch(self, hero_id, faith_amount, max_faith).await
    }

    pub async fn add_faith_by_uids(
        &self,
        hero_uids: &[i64],
        faith_amount: i32,
        max_faith: i32,
    ) -> Result<Vec<i32>> {
        let mut tx = self.pool.begin().await?;
        let changed = Self::add_faith_by_uids_in_transaction(
            &mut tx,
            self.user_id,
            hero_uids,
            faith_amount,
            max_faith,
        )
        .await?;
        tx.commit().await?;
        Ok(changed)
    }

    pub async fn add_faith_by_uids_in_transaction(
        tx: &mut Transaction<'_, Sqlite>,
        user_id: i64,
        hero_uids: &[i64],
        faith_amount: i32,
        max_faith: i32,
    ) -> Result<Vec<i32>> {
        if faith_amount <= 0 || max_faith <= 0 {
            return Ok(Vec::new());
        }

        let mut hero_uids = hero_uids.to_vec();
        hero_uids.sort_unstable();
        hero_uids.dedup();

        let mut changed = Vec::new();
        for hero_uid in hero_uids {
            let hero_id = sqlx::query_scalar::<_, i32>(
                "UPDATE heroes
                 SET faith = MIN(faith + ?, ?)
                 WHERE user_id = ? AND uid = ? AND faith < ?
                 RETURNING hero_id",
            )
            .bind(faith_amount)
            .bind(max_faith)
            .bind(user_id)
            .bind(hero_uid)
            .bind(max_faith)
            .fetch_optional(&mut **tx)
            .await?;
            changed.extend(hero_id);
        }
        Ok(changed)
    }

    pub async fn set_favor(&self, hero_id: i32, is_favor: bool) -> Result<()> {
        HeroModel::<HeroData>::set_favor(self, hero_id, is_favor).await
    }
}
