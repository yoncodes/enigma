use super::*;

impl UserHeroModel {
    pub async fn get_birthdays(&self) -> Result<Vec<(i32, i32)>> {
        HeroModel::<HeroData>::birthdays(self).await
    }

    pub async fn unlock_voice(&self, hero_id: i32, voice_id: i32) -> Result<bool> {
        if !config::configs::get()
            .character_voice
            .iter()
            .any(|voice| voice.hero_id == hero_id && voice.audio == voice_id)
        {
            return Err(anyhow!(
                "voice {voice_id} does not belong to hero {hero_id}"
            ));
        }

        let hero_uid = self.get_hero(hero_id).await?.record.uid;
        let result =
            sqlx::query("INSERT OR IGNORE INTO hero_voices (hero_uid, voice_id) VALUES (?, ?)")
                .bind(hero_uid)
                .bind(voice_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn unlock_item_with_currency_reward(
        &self,
        hero_uid: i64,
        item_id: i32,
        currency_id: i32,
        amount: i32,
        limit: i32,
    ) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            "INSERT OR IGNORE INTO hero_item_unlocks (hero_uid, item_id) VALUES (?, ?)",
        )
        .bind(hero_uid)
        .bind(item_id)
        .execute(&mut *tx)
        .await?;
        if inserted.rows_affected() != 1 {
            return Ok(false);
        }

        let rewarded = sqlx::query(
            "INSERT INTO currencies
                 (user_id, currency_id, quantity, last_recover_time, expired_time)
             VALUES (?, ?, ?, ?, 0)
             ON CONFLICT(user_id, currency_id) DO UPDATE SET
                 quantity = quantity + excluded.quantity,
                 last_recover_time = excluded.last_recover_time
             WHERE quantity + excluded.quantity <= ?",
        )
        .bind(self.user_id)
        .bind(currency_id)
        .bind(amount)
        .bind(common::time::ServerTime::now_ms())
        .bind(limit)
        .execute(&mut *tx)
        .await?;
        if rewarded.rows_affected() != 1 {
            return Ok(false);
        }

        tx.commit().await?;
        Ok(true)
    }
}
