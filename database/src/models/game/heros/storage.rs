use super::types::{initial_voice_ids, passive_skill_levels};
use super::*;
use sqlx::FromRow;

#[async_trait::async_trait]
impl HeroModel<HeroData> for UserHeroModel {
    async fn get(&self, hero_id: i32) -> Result<HeroData> {
        let hero_record =
            sqlx::query_as::<_, Hero>("SELECT * FROM heroes WHERE user_id = ? AND hero_id = ?")
                .bind(self.user_id)
                .bind(hero_id)
                .fetch_one(&self.pool)
                .await?;

        let hero_uid = hero_record.uid;

        let passive_skill_levels = passive_skill_levels(&hero_record);

        let voices: Vec<i32> =
            sqlx::query_scalar("SELECT voice_id FROM hero_voices WHERE hero_uid = ?")
                .bind(hero_uid)
                .fetch_all(&self.pool)
                .await?;

        let voices_heard: Vec<i32> =
            sqlx::query_scalar("SELECT voice_id FROM hero_voices_heard WHERE hero_uid = ?")
                .bind(hero_uid)
                .fetch_all(&self.pool)
                .await?;

        let skin_list = sqlx::query_as::<_, HeroSkin>(
            "SELECT hero_uid, skin, expire_sec FROM hero_skins WHERE hero_uid = ?",
        )
        .bind(hero_uid)
        .fetch_all(&self.pool)
        .await?;

        let item_unlocks: Vec<i32> =
            sqlx::query_scalar("SELECT item_id FROM hero_item_unlocks WHERE hero_uid = ?")
                .bind(hero_uid)
                .fetch_all(&self.pool)
                .await?;

        let talent_cubes = sqlx::query_as::<_, HeroTalentCube>(
            "SELECT hero_uid, cube_id, direction, pos_x, pos_y FROM hero_talent_cubes WHERE hero_uid = ?"
        )
        .bind(hero_uid)
        .fetch_all(&self.pool)
        .await?;

        let templates = sqlx::query_as::<_, HeroTalentTemplate>(
            "SELECT id, hero_uid, template_id, name, style FROM hero_talent_templates WHERE hero_uid = ?"
        )
        .bind(hero_uid)
        .fetch_all(&self.pool)
        .await?;

        let mut talent_templates = Vec::new();
        for template in templates {
            let template_cubes = sqlx::query_as::<_, HeroTalentCube>(
                "SELECT 0 as hero_uid, cube_id, direction, pos_x, pos_y
                 FROM hero_talent_template_cubes WHERE template_row_id = ?",
            )
            .bind(template.id)
            .fetch_all(&self.pool)
            .await?;

            talent_templates.push((template, template_cubes));
        }

        let destiny_stone_unlocks: Vec<i32> = sqlx::query_scalar(
            "SELECT stone_id FROM hero_destiny_stone_unlocks WHERE hero_uid = ?",
        )
        .bind(hero_uid)
        .fetch_all(&self.pool)
        .await?;

        Ok(HeroData {
            record: hero_record,
            passive_skill_levels,
            voices,
            voices_heard,
            skin_list,
            item_unlocks,
            talent_cubes,
            talent_templates,
            destiny_stone_unlocks,
        })
    }

    async fn get_uid(&self, hero_uid: i64) -> Result<HeroData> {
        let hero_record =
            sqlx::query_as::<_, Hero>("SELECT * FROM heroes WHERE user_id = ? AND uid = ?")
                .bind(self.user_id)
                .bind(hero_uid)
                .fetch_one(&self.pool)
                .await?;

        let hero_uid = hero_record.uid;

        let passive_skill_levels = passive_skill_levels(&hero_record);

        let voices: Vec<i32> =
            sqlx::query_scalar("SELECT voice_id FROM hero_voices WHERE hero_uid = ?")
                .bind(hero_uid)
                .fetch_all(&self.pool)
                .await?;

        let voices_heard: Vec<i32> =
            sqlx::query_scalar("SELECT voice_id FROM hero_voices_heard WHERE hero_uid = ?")
                .bind(hero_uid)
                .fetch_all(&self.pool)
                .await?;

        let skin_list = sqlx::query_as::<_, HeroSkin>(
            "SELECT hero_uid, skin, expire_sec FROM hero_skins WHERE hero_uid = ?",
        )
        .bind(hero_uid)
        .fetch_all(&self.pool)
        .await?;

        let item_unlocks: Vec<i32> =
            sqlx::query_scalar("SELECT item_id FROM hero_item_unlocks WHERE hero_uid = ?")
                .bind(hero_uid)
                .fetch_all(&self.pool)
                .await?;

        let talent_cubes = sqlx::query_as::<_, HeroTalentCube>(
            "SELECT hero_uid, cube_id, direction, pos_x, pos_y FROM hero_talent_cubes WHERE hero_uid = ?"
        )
        .bind(hero_uid)
        .fetch_all(&self.pool)
        .await?;

        let templates = sqlx::query_as::<_, HeroTalentTemplate>(
            "SELECT id, hero_uid, template_id, name, style FROM hero_talent_templates WHERE hero_uid = ?"
        )
        .bind(hero_uid)
        .fetch_all(&self.pool)
        .await?;

        let mut talent_templates = Vec::new();
        for template in templates {
            let template_cubes = sqlx::query_as::<_, HeroTalentCube>(
                "SELECT 0 as hero_uid, cube_id, direction, pos_x, pos_y
                 FROM hero_talent_template_cubes WHERE template_row_id = ?",
            )
            .bind(template.id)
            .fetch_all(&self.pool)
            .await?;

            talent_templates.push((template, template_cubes));
        }

        let destiny_stone_unlocks: Vec<i32> = sqlx::query_scalar(
            "SELECT stone_id FROM hero_destiny_stone_unlocks WHERE hero_uid = ?",
        )
        .bind(hero_uid)
        .fetch_all(&self.pool)
        .await?;

        Ok(HeroData {
            record: hero_record,
            passive_skill_levels,
            voices,
            voices_heard,
            skin_list,
            item_unlocks,
            talent_cubes,
            talent_templates,
            destiny_stone_unlocks,
        })
    }

    async fn get_all(&self) -> Result<Vec<HeroData>> {
        let heroes =
            sqlx::query_as::<_, Hero>("SELECT * FROM heroes WHERE user_id = ?1 ORDER BY uid")
                .bind(self.user_id)
                .fetch_all(&self.pool)
                .await?;

        let mut result = Vec::new();

        for hero_record in heroes {
            let hero_uid = hero_record.uid;

            let passive_skill_levels = passive_skill_levels(&hero_record);

            let voices: Vec<i32> =
                sqlx::query_scalar("SELECT voice_id FROM hero_voices WHERE hero_uid = ?1")
                    .bind(hero_uid)
                    .fetch_all(&self.pool)
                    .await?;

            let voices_heard: Vec<i32> =
                sqlx::query_scalar("SELECT voice_id FROM hero_voices_heard WHERE hero_uid = ?1")
                    .bind(hero_uid)
                    .fetch_all(&self.pool)
                    .await?;

            let skin_list = sqlx::query_as::<_, HeroSkin>(
                "SELECT hero_uid, skin, expire_sec FROM hero_skins WHERE hero_uid = ?1",
            )
            .bind(hero_uid)
            .fetch_all(&self.pool)
            .await?;

            let item_unlocks: Vec<i32> =
                sqlx::query_scalar("SELECT item_id FROM hero_item_unlocks WHERE hero_uid = ?1")
                    .bind(hero_uid)
                    .fetch_all(&self.pool)
                    .await?;

            let talent_cubes = sqlx::query_as::<_, HeroTalentCube>(
                "SELECT hero_uid, cube_id, direction, pos_x, pos_y FROM hero_talent_cubes WHERE hero_uid = ?1"
            )
            .bind(hero_uid)
            .fetch_all(&self.pool)
            .await?;

            let templates = sqlx::query_as::<_, HeroTalentTemplate>(
                "SELECT id, hero_uid, template_id, name, style FROM hero_talent_templates WHERE hero_uid = ?1"
            )
            .bind(hero_uid)
            .fetch_all(&self.pool)
            .await?;

            let mut talent_templates = Vec::new();
            for template in templates {
                let template_cubes = sqlx::query_as::<_, HeroTalentCube>(
                    "SELECT 0 as hero_uid, cube_id, direction, pos_x, pos_y
                     FROM hero_talent_template_cubes WHERE template_row_id = ?1",
                )
                .bind(template.id)
                .fetch_all(&self.pool)
                .await?;

                talent_templates.push((template, template_cubes));
            }

            let destiny_stone_unlocks: Vec<i32> = sqlx::query_scalar(
                "SELECT stone_id FROM hero_destiny_stone_unlocks WHERE hero_uid = ?1",
            )
            .bind(hero_uid)
            .fetch_all(&self.pool)
            .await?;

            result.push(HeroData {
                record: hero_record,
                passive_skill_levels,
                voices,
                voices_heard,
                skin_list,
                item_unlocks,
                talent_cubes,
                talent_templates,
                destiny_stone_unlocks,
            });
        }

        Ok(result)
    }

    async fn has_hero(&self, hero_id: i32) -> Result<bool> {
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM heroes WHERE user_id = ? AND hero_id = ?",
        )
        .bind(self.user_id)
        .bind(hero_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(exists > 0)
    }

    async fn hero_duplicate(&self, hero_id: i32) -> Result<i32> {
        sqlx::query(
            r#"
            UPDATE heroes
            SET duplicate_count = duplicate_count + 1
            WHERE user_id = ? AND hero_id = ?
            "#,
        )
        .bind(self.user_id)
        .bind(hero_id)
        .execute(&self.pool)
        .await?;

        let new_count = sqlx::query_scalar::<_, i32>(
            "SELECT duplicate_count FROM heroes WHERE user_id = ? AND hero_id = ?",
        )
        .bind(self.user_id)
        .bind(hero_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(new_count)
    }

    async fn create_hero(&self, tx: &mut Transaction<'_, Sqlite>, hero_id: i32) -> Result<i64> {
        let game_data = config::configs::get();
        let now = common::time::ServerTime::now_ms();

        let last_hero_uid: Option<i64> =
            sqlx::query_scalar("SELECT uid FROM heroes ORDER BY uid DESC LIMIT 1")
                .fetch_optional(&mut **tx)
                .await?;

        let hero_uid = match last_hero_uid {
            Some(uid) => uid + 1,
            None => 20000001,
        };

        let character = game_data
            .character
            .get(hero_id)
            .filter(|character| character.id != 3029 && character.id != 9998) // npc
            .ok_or_else(|| anyhow!("hero {hero_id} has no character config"))?;

        let hero_skin = character.skin_id;
        let rare = character.rare as usize;

        let level = game_data
            .starting_character_level(hero_id)
            .map(|row| row.level)
            .ok_or_else(|| anyhow!("hero {hero_id} has no character_level config"))?;

        let min_rank = game_data
            .starting_character_rank(hero_id)
            .map(|row| row.rank)
            .ok_or_else(|| anyhow!("hero {hero_id} has no character_rank config"))?;

        let default_skin = game_data
            .default_character_skin(hero_id)
            .map(|row| row.id)
            .unwrap_or(hero_skin);

        let (destiny_rank, destiny_level, destiny_stone, red_dot_type) = (0, 0, 0, 0);

        let starting_talent = game_data
            .character_talent(hero_id, 1)
            .map(|row| row.talent_id)
            .unwrap_or(1);

        sqlx::query(
            r#"
            INSERT INTO heroes (
                uid, user_id, hero_id, create_time,
                level, exp, rank, breakthrough, skin, faith,
                active_skill_level, ex_skill_level, is_new, talent,
                default_equip_uid, duplicate_count, use_talent_template_id,
                talent_style_unlock, talent_style_red, is_favor,
                destiny_rank, destiny_level, destiny_stone, red_dot, extra_str
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                ?21, ?22, ?23, ?24, ?25
            )
            "#,
        )
        .bind(hero_uid)
        .bind(self.user_id)
        .bind(hero_id)
        .bind(now)
        .bind(level) // Level 1
        .bind(0) // Starting exp
        .bind(min_rank) // Starting rank
        .bind(0) // No breakthrough
        .bind(default_skin)
        .bind(0) // Starting faith
        .bind(1) // Active skill level 1
        .bind(0) // Ex skill level 0
        .bind(true) // is_new (true for new heroes)
        .bind(starting_talent) // talent
        .bind(0) // default_equip_uid = 0
        .bind(0) // duplicate_count
        .bind(1) // use_talent_template_id
        .bind(0) // talent_style_unlock
        .bind(0) // talent_style_red
        .bind(false) // is_favor
        .bind(destiny_rank) // destiny_rank (0)
        .bind(destiny_level) // destiny_level (0)
        .bind(destiny_stone) // destiny_stone
        .bind(red_dot_type) // red_dot
        .bind("") // Hero-specific extras are selected through their RPCs.
        .execute(&mut **tx)
        .await?;

        self.attach_owned_skins_in_transaction(tx, hero_uid, hero_id)
            .await?;

        for voice_id in initial_voice_ids(game_data, hero_id) {
            sqlx::query("INSERT INTO hero_voices (hero_uid, voice_id) VALUES (?, ?)")
                .bind(hero_uid)
                .bind(voice_id)
                .execute(&mut **tx)
                .await?;
        }

        sqlx::query(
            "INSERT INTO hero_birthday_info (user_id, hero_id, birthday_count) VALUES (?, ?, ?)
             ON CONFLICT(user_id, hero_id) DO NOTHING",
        )
        .bind(self.user_id)
        .bind(hero_id)
        .bind(0) // Starting at 0 birthday celebrations
        .execute(&mut **tx)
        .await?;

        let talent_config = game_data.character_talent(hero_id, 1);

        if let Some(talent) = talent_config {
            let talent_scheme = game_data.talent_scheme(talent.talent_id, talent.talent_mould);

            if let Some(scheme) = talent_scheme {
                let cubes: Vec<(i32, i32, i32, i32)> = scheme
                    .talen_scheme
                    .split('#')
                    .filter_map(|cube_str| {
                        let parts: Vec<&str> = cube_str.split(',').collect();
                        if parts.len() == 4 {
                            let cube_id = parts[0].parse::<i32>().ok()?;
                            let direction = parts[1].parse::<i32>().ok()?;
                            let pos_x = parts[2].parse::<i32>().ok()?;
                            let pos_y = parts[3].parse::<i32>().ok()?;
                            Some((cube_id, direction, pos_x, pos_y))
                        } else {
                            None
                        }
                    })
                    .collect();

                for (cube_id, direction, pos_x, pos_y) in &cubes {
                    sqlx::query(
                        "INSERT INTO hero_talent_cubes (hero_uid, cube_id, direction, pos_x, pos_y) VALUES (?, ?, ?, ?, ?)"
                    )
                    .bind(hero_uid)
                    .bind(cube_id)
                    .bind(direction)
                    .bind(pos_x)
                    .bind(pos_y)
                    .execute(&mut **tx)
                    .await?;
                }

                tracing::info!(
                    "Inserted {} talent cubes for hero {} talent 1",
                    cubes.len(),
                    hero_id
                );
            }
        }

        // Insert talent templates
        for template_id in 1..=4 {
            let result = sqlx::query(
                "INSERT INTO hero_talent_templates (hero_uid, template_id, name, style) VALUES (?, ?, ?, ?)"
            )
            .bind(hero_uid)
            .bind(template_id)
            .bind("")
            .bind(0)
            .execute(&mut **tx)
            .await?;

            let template_row_id = result.last_insert_rowid();

            // Template #1 gets the same cubes as active (saved preset)
            if template_id == 1
                && talent_config.is_some()
                && let Some(talent) = talent_config
            {
                let talent_scheme = game_data.talent_scheme(talent.talent_id, talent.talent_mould);

                if let Some(scheme) = talent_scheme {
                    let cubes: Vec<(i32, i32, i32, i32)> = scheme
                        .talen_scheme
                        .split('#')
                        .filter_map(|cube_str| {
                            let parts: Vec<&str> = cube_str.split(',').collect();
                            if parts.len() == 4 {
                                Some((
                                    parts[0].parse().ok()?,
                                    parts[1].parse().ok()?,
                                    parts[2].parse().ok()?,
                                    parts[3].parse().ok()?,
                                ))
                            } else {
                                None
                            }
                        })
                        .collect();

                    for (cube_id, direction, pos_x, pos_y) in &cubes {
                        sqlx::query(
                                "INSERT INTO hero_talent_template_cubes (template_row_id, cube_id, direction, pos_x, pos_y) VALUES (?, ?, ?, ?, ?)"
                            )
                            .bind(template_row_id)
                            .bind(cube_id)
                            .bind(direction)
                            .bind(pos_x)
                            .bind(pos_y)
                            .execute(&mut **tx)
                            .await?;
                    }
                }
            }
        }

        let rarity_column = match rare {
            1 => "hero_rare_nn_count",
            2 => "hero_rare_n_count",
            3 => "hero_rare_r_count",
            4 => "hero_rare_sr_count",
            5 => "hero_rare_ssr_count",
            _ => "",
        };
        if !rarity_column.is_empty() {
            sqlx::query(&format!(
                "UPDATE player_info
                 SET {rarity_column} = {rarity_column} + 1, updated_at = ?
                 WHERE player_id = ?"
            ))
            .bind(now)
            .bind(self.user_id)
            .execute(&mut **tx)
            .await?;
        }

        tracing::info!(
            "Created hero {} (uid {}) for user {}",
            hero_id,
            hero_uid,
            self.user_id
        );

        Ok(hero_uid)
    }

    async fn hero_count(&self, rarity: usize, now: i64) -> Result<()> {
        let rarity_column = match rarity {
            1 => "hero_rare_nn_count",
            2 => "hero_rare_n_count",
            3 => "hero_rare_r_count",
            4 => "hero_rare_sr_count",
            5 => "hero_rare_ssr_count",
            _ => return Ok(()),
        };

        sqlx::query(&format!(
            r#"
            UPDATE player_info
            SET {} = {} + 1,
                updated_at = ?
            WHERE player_id = ?
            "#,
            rarity_column, rarity_column
        ))
        .bind(now)
        .bind(self.user_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn special_equipped_gear(&self, hero_id: i32, extra_str: String) -> Result<()> {
        let hero_data = self.get(hero_id).await?;
        sqlx::query("UPDATE heroes SET extra_str = ? WHERE uid = ?")
            .bind(&extra_str)
            .bind(hero_data.record.uid)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn equipped_gear(&self, hero_id: i32, equip_uid: i64) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE heroes SET default_equip_uid = ?
             WHERE hero_id = ? AND user_id = ?
               AND (? = 0 OR EXISTS(
                   SELECT 1 FROM equipment
                   WHERE equipment.uid = ? AND equipment.user_id = heroes.user_id
               ))",
        )
        .bind(equip_uid)
        .bind(hero_id)
        .bind(self.user_id)
        .bind(equip_uid)
        .bind(equip_uid)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }

    async fn use_touch(
        &self,
        hero_id: i32,
        faith_amount: i32,
        max_faith: i32,
    ) -> Result<Option<i32>> {
        let mut tx = self.pool.begin().await?;
        let new_count = sqlx::query_scalar::<_, i32>(
            "UPDATE hero_touch_count
             SET touch_count_left = touch_count_left - 1
             WHERE user_id = ? AND touch_count_left > 0
             RETURNING touch_count_left",
        )
        .bind(self.user_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(new_count) = new_count else {
            tx.rollback().await?;
            return Ok(None);
        };

        let updated = sqlx::query(
            "UPDATE heroes
             SET faith = MIN(faith + ?, ?)
             WHERE user_id = ? AND hero_id = ?",
        )
        .bind(faith_amount.max(0))
        .bind(max_faith.max(0))
        .bind(self.user_id)
        .bind(hero_id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            tx.rollback().await?;
            return Err(anyhow!(
                "hero {hero_id} is not owned by user {}",
                self.user_id
            ));
        }

        tx.commit().await?;
        Ok(Some(new_count))
    }

    async fn touch_count(&self) -> Result<Option<i32>> {
        let count: Option<i32> =
            sqlx::query_scalar("SELECT touch_count_left FROM hero_touch_count WHERE user_id = ?1")
                .bind(self.user_id)
                .fetch_optional(&self.pool)
                .await?;

        Ok(count)
    }

    async fn skin(&self, hero_id: i32, skin_id: i32) -> Result<bool> {
        let tables = config::configs::get();
        if tables
            .skin
            .get(skin_id)
            .is_none_or(|skin| skin.character_id != hero_id)
        {
            return Ok(false);
        }
        let is_default = tables
            .default_character_skin(hero_id)
            .is_some_and(|skin| skin.id == skin_id);

        let result = sqlx::query(
            "UPDATE heroes SET skin = ?
             WHERE hero_id = ? AND user_id = ?
               AND (? OR EXISTS(
                   SELECT 1 FROM hero_all_skins
                   WHERE hero_all_skins.user_id = heroes.user_id
                     AND hero_all_skins.skin_id = ?
               ))",
        )
        .bind(skin_id)
        .bind(hero_id)
        .bind(self.user_id)
        .bind(is_default)
        .bind(skin_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }

    async fn skins(&self) -> Result<Vec<i32>> {
        let skins: Vec<i32> =
            sqlx::query_scalar("SELECT skin_id FROM hero_all_skins WHERE user_id = ?1")
                .bind(self.user_id)
                .fetch_all(&self.pool)
                .await?;

        Ok(skins)
    }

    async fn birthdays(&self) -> Result<Vec<(i32, i32)>> {
        let info: Vec<(i32, i32)> = sqlx::query_as(
            "SELECT hero_id, birthday_count FROM hero_birthday_info WHERE user_id = ?1",
        )
        .bind(self.user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(info)
    }

    async fn destiny_stone(&self, hero_id: i32, stone_id: i32) -> Result<()> {
        let hero_data = self.get(hero_id).await?;
        sqlx::query("UPDATE heroes SET destiny_stone = ? WHERE uid = ?")
            .bind(stone_id)
            .bind(hero_data.record.uid)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn update_destiny_progress(&self, hero_id: i32, rank: i32, level: i32) -> Result<()> {
        let hero_data = self.get(hero_id).await?;
        sqlx::query("UPDATE heroes SET destiny_rank = ?, destiny_level = ? WHERE uid = ?")
            .bind(rank)
            .bind(level)
            .bind(hero_data.record.uid)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn unlock_destiny_stone(&self, hero_id: i32, stone_id: i32) -> Result<bool> {
        let hero_data = self.get(hero_id).await?;
        let result = sqlx::query(
            "INSERT OR IGNORE INTO hero_destiny_stone_unlocks (hero_uid, stone_id) VALUES (?, ?)",
        )
        .bind(hero_data.record.uid)
        .bind(stone_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }

    async fn level_up(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        hero_id: i32,
        current_level: i32,
        new_level: i32,
    ) -> Result<bool> {
        let hero_data = self.get(hero_id).await?;

        let updated = sqlx::query("UPDATE heroes SET level = ? WHERE uid = ? AND level = ?")
            .bind(new_level)
            .bind(hero_data.record.uid)
            .bind(current_level)
            .execute(&mut **tx)
            .await?;

        Ok(updated.rows_affected() == 1)
    }

    async fn read_hero_red_dot(&self, hero_id: i32, red_dot_type: i32) -> Result<i32> {
        let hero_data = self.get(hero_id).await?;
        let red_dot = sqlx::query_scalar(
            "UPDATE heroes
             SET red_dot = (red_dot & ~1) | ((1 << (?1 + 1)) - 2)
             WHERE uid = ?2 AND user_id = ?3
             RETURNING red_dot",
        )
        .bind(red_dot_type)
        .bind(hero_data.record.uid)
        .bind(self.user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(red_dot)
    }

    async fn upgrade_ex_skill(&self, hero_id: i32, levels: i32) -> Result<()> {
        let hero_data = self.get(hero_id).await?;
        let new_level = (hero_data.record.ex_skill_level + levels).min(5);

        sqlx::query("UPDATE heroes SET ex_skill_level = ? WHERE uid = ? AND user_id = ?")
            .bind(new_level)
            .bind(hero_data.record.uid)
            .bind(self.user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn set_favor(&self, hero_id: i32, is_favor: bool) -> Result<()> {
        let hero_data = self.get(hero_id).await?;

        sqlx::query("UPDATE heroes SET is_favor = ? WHERE uid = ? AND user_id = ?")
            .bind(is_favor)
            .bind(hero_data.record.uid)
            .bind(self.user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn unmark_new(&self, hero_id: i32) -> Result<()> {
        let hero_data = self.get(hero_id).await?;

        sqlx::query("UPDATE heroes SET is_new = 0 WHERE uid = ? AND user_id = ?")
            .bind(hero_data.record.uid)
            .bind(self.user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn set_show_hero(&self, hero_uids: Vec<i64>) -> Result<()> {
        for (slot_idx, uid) in hero_uids.into_iter().enumerate() {
            let display_order = slot_idx as i32;

            if uid == 0 {
                sqlx::query(
                    r#"
                    DELETE FROM player_show_heroes
                    WHERE player_id = ? AND display_order = ?
                    "#,
                )
                .bind(self.user_id)
                .bind(display_order)
                .execute(&self.pool)
                .await?;

                continue;
            }

            #[derive(FromRow)]
            struct HeroRow {
                hero_id: i32,
                level: i32,
                rank: i32,
                ex_skill_level: i32,
                skin: i32,
            }

            let hero = sqlx::query_as::<_, HeroRow>(
                "
                SELECT
                    hero_id,
                    level,
                    rank,
                    ex_skill_level,
                    skin
                FROM heroes
                WHERE uid = ? AND user_id = ?
                ",
            )
            .bind(uid)
            .bind(self.user_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Invalid hero uid {} for user {}", uid, self.user_id))?;

            sqlx::query(
                r#"
                INSERT INTO player_show_heroes (
                    player_id,
                    hero_id,
                    level,
                    rank,
                    ex_skill_level,
                    skin,
                    display_order
                )
                VALUES (?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(player_id, display_order)
                DO UPDATE SET
                    hero_id = excluded.hero_id,
                    level = excluded.level,
                    rank = excluded.rank,
                    ex_skill_level = excluded.ex_skill_level,
                    skin = excluded.skin
                "#,
            )
            .bind(self.user_id)
            .bind(hero.hero_id)
            .bind(hero.level)
            .bind(hero.rank)
            .bind(hero.ex_skill_level)
            .bind(hero.skin)
            .bind(display_order)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    async fn talent_style_read(&self, hero_id: i32) -> Result<()> {
        UserHeroModel::talent_style_read_impl(self, hero_id).await
    }

    async fn update_talent(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        hero_id: i32,
        current_talent: i32,
        talent_id: i32,
    ) -> Result<bool> {
        UserHeroModel::update_talent_impl(self, tx, hero_id, current_talent, talent_id).await
    }

    async fn remove_talent_cube(
        &self,
        hero_id: i32,
        template_id: i32,
        pos_x: i32,
        pos_y: i32,
    ) -> Result<()> {
        UserHeroModel::remove_talent_cube_impl(self, hero_id, template_id, pos_x, pos_y).await
    }

    async fn place_talent_cube(
        &self,
        hero_id: i32,
        template_id: i32,
        cube_id: i32,
        direction: i32,
        pos_x: i32,
        pos_y: i32,
    ) -> Result<()> {
        UserHeroModel::place_talent_cube_impl(
            self,
            hero_id,
            template_id,
            cube_id,
            direction,
            pos_x,
            pos_y,
        )
        .await
    }

    async fn sync_active_talent_cubes(
        &self,
        hero_id: i32,
        template_id: i32,
        get_cube: Option<(i32, i32)>,
        put_cube: Option<(i32, i32, i32, i32)>,
    ) -> Result<()> {
        UserHeroModel::sync_active_talent_cubes_impl(self, hero_id, template_id, get_cube, put_cube)
            .await
    }

    async fn replace_talent_cubes(
        &self,
        hero_id: i32,
        template_id: i32,
        cubes: Vec<(i32, i32, i32, i32)>,
    ) -> Result<sonettobuf::TalentTemplateInfo> {
        UserHeroModel::replace_talent_cubes_impl(self, hero_id, template_id, cubes).await
    }

    async fn get_template_info(
        &self,
        hero_id: i32,
        template_id: i32,
    ) -> Result<sonettobuf::TalentTemplateInfo> {
        UserHeroModel::get_template_info_impl(self, hero_id, template_id).await
    }

    async fn rename_talent_template(
        &self,
        hero_id: i32,
        template_id: i32,
        name: &str,
    ) -> Result<sonettobuf::TalentTemplateInfo> {
        UserHeroModel::rename_talent_template_impl(self, hero_id, template_id, name).await
    }

    async fn load_talent_scheme(
        &self,
        hero_id: i32,
        talent_id: i32,
        talent_mould: i32,
        template_id: i32,
    ) -> Result<sonettobuf::TalentTemplateInfo> {
        UserHeroModel::load_talent_scheme_impl(self, hero_id, talent_id, talent_mould, template_id)
            .await
    }

    async fn has_talent_style(&self, hero_id: i32, style: i32) -> Result<bool> {
        UserHeroModel::has_talent_style_impl(self, hero_id, style).await
    }

    async fn unlock_talent_style(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        hero_id: i32,
        style: i32,
    ) -> Result<bool> {
        UserHeroModel::unlock_talent_style_impl(self, tx, hero_id, style).await
    }

    async fn apply_talent_style(&self, hero_id: i32, template_id: i32, style: i32) -> Result<()> {
        UserHeroModel::apply_talent_style_impl(self, hero_id, template_id, style).await
    }

    async fn switch_talent_template(
        &self,
        hero_id: i32,
        template_id: i32,
    ) -> Result<sonettobuf::TalentTemplateInfo> {
        UserHeroModel::switch_talent_template_impl(self, hero_id, template_id).await
    }
}
