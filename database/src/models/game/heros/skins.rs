use super::*;

impl UserHeroModel {
    pub async fn update_skin(&self, hero_uid: i32, skin_id: i32) -> Result<bool> {
        HeroModel::<HeroData>::skin(self, hero_uid, skin_id).await
    }

    pub async fn get_skins(&self) -> Result<Vec<i32>> {
        HeroModel::<HeroData>::skins(self).await
    }

    pub async fn has_skin(&self, skin_id: i32) -> Result<bool> {
        let has_skin: Option<i32> =
            sqlx::query_scalar("SELECT 1 FROM hero_all_skins WHERE user_id = ? AND skin_id = ?")
                .bind(self.user_id)
                .bind(skin_id)
                .fetch_optional(&self.pool)
                .await?;

        Ok(has_skin.is_some())
    }

    pub async fn unlock_skin(&self, skin_id: i32) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        let unlocked = self.unlock_skin_in_transaction(&mut tx, skin_id).await?;
        tx.commit().await?;
        Ok(unlocked)
    }

    pub async fn unlock_skins_without_equipping(&self, skin_ids: &[i32]) -> Result<Vec<i32>> {
        let mut tx = self.pool.begin().await?;
        let mut unlocked = Vec::new();
        for &skin_id in skin_ids {
            let skin = config::configs::get()
                .skin
                .get(skin_id)
                .ok_or_else(|| anyhow!("skin {} not found", skin_id))?;
            let inserted = sqlx::query(
                "INSERT OR IGNORE INTO hero_all_skins (user_id, skin_id) VALUES (?, ?)",
            )
            .bind(self.user_id)
            .bind(skin_id)
            .execute(&mut *tx)
            .await?;
            if inserted.rows_affected() == 0 {
                continue;
            }

            let hero_uid: Option<i64> =
                sqlx::query_scalar("SELECT uid FROM heroes WHERE user_id = ? AND hero_id = ?")
                    .bind(self.user_id)
                    .bind(skin.character_id)
                    .fetch_optional(&mut *tx)
                    .await?;
            if let Some(hero_uid) = hero_uid {
                sqlx::query(
                    "INSERT OR IGNORE INTO hero_skins (hero_uid, skin, expire_sec) VALUES (?, ?, 0)",
                )
                .bind(hero_uid)
                .bind(skin_id)
                .execute(&mut *tx)
                .await?;
            }
            unlocked.push(skin_id);
        }
        tx.commit().await?;
        Ok(unlocked)
    }

    pub async fn unlock_skin_in_transaction(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        skin_id: i32,
    ) -> Result<bool> {
        let skin = config::configs::get()
            .skin
            .get(skin_id)
            .ok_or_else(|| anyhow!("skin {} not found", skin_id))?;
        let inserted =
            sqlx::query("INSERT OR IGNORE INTO hero_all_skins (user_id, skin_id) VALUES (?, ?)")
                .bind(self.user_id)
                .bind(skin_id)
                .execute(&mut **tx)
                .await?;
        if inserted.rows_affected() == 0 {
            return Ok(false);
        }

        let hero_uid: Option<i64> =
            sqlx::query_scalar("SELECT uid FROM heroes WHERE user_id = ? AND hero_id = ?")
                .bind(self.user_id)
                .bind(skin.character_id)
                .fetch_optional(&mut **tx)
                .await?;
        if let Some(hero_uid) = hero_uid {
            sqlx::query(
                "INSERT OR IGNORE INTO hero_skins (hero_uid, skin, expire_sec) VALUES (?, ?, 0)",
            )
            .bind(hero_uid)
            .bind(skin_id)
            .execute(&mut **tx)
            .await?;
            sqlx::query("UPDATE heroes SET skin = ? WHERE uid = ? AND user_id = ?")
                .bind(skin_id)
                .bind(hero_uid)
                .bind(self.user_id)
                .execute(&mut **tx)
                .await?;
        }
        Ok(true)
    }

    pub(super) async fn attach_owned_skins_in_transaction(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        hero_uid: i64,
        hero_id: i32,
    ) -> Result<()> {
        let owned_skins: Vec<i32> =
            sqlx::query_scalar("SELECT skin_id FROM hero_all_skins WHERE user_id = ?")
                .bind(self.user_id)
                .fetch_all(&mut **tx)
                .await?;

        for skin_id in owned_skins.into_iter().filter(|skin_id| {
            config::configs::get()
                .skin
                .get(*skin_id)
                .is_some_and(|skin| skin.character_id == hero_id)
        }) {
            sqlx::query(
                "INSERT OR IGNORE INTO hero_skins (hero_uid, skin, expire_sec) VALUES (?, ?, 0)",
            )
            .bind(hero_uid)
            .bind(skin_id)
            .execute(&mut **tx)
            .await?;
        }

        Ok(())
    }
}
