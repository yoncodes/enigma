use super::*;

impl UserHeroModel {
    pub(super) async fn talent_style_read_impl(&self, hero_id: i32) -> Result<()> {
        let hero_data = self.get(hero_id).await?;

        sqlx::query("UPDATE heroes SET talent_style_red = 0 WHERE uid = ? AND user_id = ?")
            .bind(hero_data.record.uid)
            .bind(self.user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub(super) async fn update_talent_impl(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        hero_id: i32,
        current_talent: i32,
        talent_id: i32,
    ) -> Result<bool> {
        let updated = sqlx::query(
            "UPDATE heroes SET talent = ?
             WHERE hero_id = ? AND user_id = ? AND talent = ?",
        )
        .bind(talent_id)
        .bind(hero_id)
        .bind(self.user_id)
        .bind(current_talent)
        .execute(&mut **tx)
        .await?;

        Ok(updated.rows_affected() == 1)
    }

    pub(super) async fn remove_talent_cube_impl(
        &self,
        hero_id: i32,
        template_id: i32,
        pos_x: i32,
        pos_y: i32,
    ) -> Result<()> {
        let hero_data = self.get(hero_id).await?;

        let template_row_id: i64 = sqlx::query_scalar(
            "SELECT id FROM hero_talent_templates WHERE hero_uid = ? AND template_id = ?",
        )
        .bind(hero_data.record.uid)
        .bind(template_id)
        .fetch_one(&self.pool)
        .await?;

        sqlx::query(
            "DELETE FROM hero_talent_template_cubes
                 WHERE template_row_id = ? AND pos_x = ? AND pos_y = ?",
        )
        .bind(template_row_id)
        .bind(pos_x)
        .bind(pos_y)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub(super) async fn place_talent_cube_impl(
        &self,
        hero_id: i32,
        template_id: i32,
        cube_id: i32,
        direction: i32,
        pos_x: i32,
        pos_y: i32,
    ) -> Result<()> {
        let hero_data = self.get(hero_id).await?;

        let template_row_id: i64 = sqlx::query_scalar(
            "SELECT id FROM hero_talent_templates WHERE hero_uid = ? AND template_id = ?",
        )
        .bind(hero_data.record.uid)
        .bind(template_id)
        .fetch_one(&self.pool)
        .await?;

        sqlx::query(
            "DELETE FROM hero_talent_template_cubes
                 WHERE template_row_id = ? AND pos_x = ? AND pos_y = ?",
        )
        .bind(template_row_id)
        .bind(pos_x)
        .bind(pos_y)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "INSERT INTO hero_talent_template_cubes
                 (template_row_id, cube_id, direction, pos_x, pos_y)
                 VALUES (?, ?, ?, ?, ?)",
        )
        .bind(template_row_id)
        .bind(cube_id)
        .bind(direction)
        .bind(pos_x)
        .bind(pos_y)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub(super) async fn sync_active_talent_cubes_impl(
        &self,
        hero_id: i32,
        template_id: i32,
        get_cube: Option<(i32, i32)>,
        put_cube: Option<(i32, i32, i32, i32)>,
    ) -> Result<()> {
        let hero_data = self.get(hero_id).await?;

        if template_id != hero_data.record.use_talent_template_id {
            return Ok(());
        }

        if let Some((pos_x, pos_y)) = get_cube {
            sqlx::query(
                "DELETE FROM hero_talent_cubes
                     WHERE hero_uid = ? AND pos_x = ? AND pos_y = ?",
            )
            .bind(hero_data.record.uid)
            .bind(pos_x)
            .bind(pos_y)
            .execute(&self.pool)
            .await?;
        }

        if let Some((cube_id, direction, pos_x, pos_y)) = put_cube {
            sqlx::query(
                "DELETE FROM hero_talent_cubes
                     WHERE hero_uid = ? AND pos_x = ? AND pos_y = ?",
            )
            .bind(hero_data.record.uid)
            .bind(pos_x)
            .bind(pos_y)
            .execute(&self.pool)
            .await?;

            sqlx::query(
                "INSERT INTO hero_talent_cubes
                     (hero_uid, cube_id, direction, pos_x, pos_y)
                     VALUES (?, ?, ?, ?, ?)",
            )
            .bind(hero_data.record.uid)
            .bind(cube_id)
            .bind(direction)
            .bind(pos_x)
            .bind(pos_y)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    pub(super) async fn replace_talent_cubes_impl(
        &self,
        hero_id: i32,
        template_id: i32,
        cubes: Vec<(i32, i32, i32, i32)>,
    ) -> Result<sonettobuf::TalentTemplateInfo> {
        let hero_data = self.get(hero_id).await?;

        let template_row_id: i64 = sqlx::query_scalar(
            "SELECT id FROM hero_talent_templates WHERE hero_uid = ? AND template_id = ?",
        )
        .bind(hero_data.record.uid)
        .bind(template_id)
        .fetch_one(&self.pool)
        .await?;

        sqlx::query("DELETE FROM hero_talent_template_cubes WHERE template_row_id = ?")
            .bind(template_row_id)
            .execute(&self.pool)
            .await?;

        for (cube_id, direction, pos_x, pos_y) in &cubes {
            sqlx::query(
                "INSERT INTO hero_talent_template_cubes
                     (template_row_id, cube_id, direction, pos_x, pos_y)
                     VALUES (?, ?, ?, ?, ?)",
            )
            .bind(template_row_id)
            .bind(cube_id)
            .bind(direction)
            .bind(pos_x)
            .bind(pos_y)
            .execute(&self.pool)
            .await?;
        }

        if template_id == hero_data.record.use_talent_template_id {
            sqlx::query("DELETE FROM hero_talent_cubes WHERE hero_uid = ?")
                .bind(hero_data.record.uid)
                .execute(&self.pool)
                .await?;

            for (cube_id, direction, pos_x, pos_y) in &cubes {
                sqlx::query(
                    "INSERT INTO hero_talent_cubes
                         (hero_uid, cube_id, direction, pos_x, pos_y)
                         VALUES (?, ?, ?, ?, ?)",
                )
                .bind(hero_data.record.uid)
                .bind(cube_id)
                .bind(direction)
                .bind(pos_x)
                .bind(pos_y)
                .execute(&self.pool)
                .await?;
            }
        }

        self.get_template_info_impl(hero_id, template_id).await
    }

    pub(super) async fn get_template_info_impl(
        &self,
        hero_id: i32,
        template_id: i32,
    ) -> Result<sonettobuf::TalentTemplateInfo> {
        let hero_data = self.get(hero_id).await?;

        let template_row_id: i64 = sqlx::query_scalar(
            "SELECT id FROM hero_talent_templates WHERE hero_uid = ? AND template_id = ?",
        )
        .bind(hero_data.record.uid)
        .bind(template_id)
        .fetch_one(&self.pool)
        .await?;

        let template_data: (String, i32) =
            sqlx::query_as("SELECT name, style FROM hero_talent_templates WHERE id = ?")
                .bind(template_row_id)
                .fetch_one(&self.pool)
                .await?;

        let cubes: Vec<(i32, i32, i32, i32)> = sqlx::query_as(
            "SELECT cube_id, direction, pos_x, pos_y
                 FROM hero_talent_template_cubes
                 WHERE template_row_id = ?",
        )
        .bind(template_row_id)
        .fetch_all(&self.pool)
        .await?;

        let talent_cube_infos: Vec<sonettobuf::TalentCubeInfo> = cubes
            .into_iter()
            .map(
                |(cube_id, direction, pos_x, pos_y)| sonettobuf::TalentCubeInfo {
                    cube_id: Some(cube_id),
                    direction: Some(direction),
                    pos_x: Some(pos_x),
                    pos_y: Some(pos_y),
                },
            )
            .collect();

        Ok(sonettobuf::TalentTemplateInfo {
            id: Some(template_id),
            talent_cube_infos,
            name: Some(template_data.0),
            style: Some(template_data.1),
        })
    }

    pub(super) async fn rename_talent_template_impl(
        &self,
        hero_id: i32,
        template_id: i32,
        name: &str,
    ) -> Result<sonettobuf::TalentTemplateInfo> {
        let hero = self.get(hero_id).await?;
        let changed = sqlx::query(
            "UPDATE hero_talent_templates SET name = ?
             WHERE hero_uid = ? AND template_id = ?",
        )
        .bind(name)
        .bind(hero.record.uid)
        .bind(template_id)
        .execute(&self.pool)
        .await?;
        if changed.rows_affected() != 1 {
            return Err(anyhow!("talent template not found"));
        }
        self.get_template_info_impl(hero_id, template_id).await
    }

    pub(super) async fn load_talent_scheme_impl(
        &self,
        hero_id: i32,
        talent_id: i32,
        talent_mould: i32,
        template_id: i32,
    ) -> Result<sonettobuf::TalentTemplateInfo> {
        let hero_data = self.get(hero_id).await?;
        let game_data = config::configs::get();

        let talent_scheme = game_data
            .talent_scheme(talent_id, talent_mould)
            .ok_or_else(|| {
                tracing::error!(
                    "Talent scheme not found for talent {} mould {}",
                    talent_id,
                    talent_mould
                );
                anyhow::anyhow!("Talent scheme not found")
            })?;

        let template_row_id: i64 = sqlx::query_scalar(
            "SELECT id FROM hero_talent_templates WHERE hero_uid = ? AND template_id = ?",
        )
        .bind(hero_data.record.uid)
        .bind(template_id)
        .fetch_one(&self.pool)
        .await?;

        let cubes: Vec<(i32, i32, i32, i32)> = talent_scheme
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

        sqlx::query("DELETE FROM hero_talent_template_cubes WHERE template_row_id = ?")
            .bind(template_row_id)
            .execute(&self.pool)
            .await?;

        for (cube_id, direction, pos_x, pos_y) in &cubes {
            sqlx::query(
                "INSERT INTO hero_talent_template_cubes
                 (template_row_id, cube_id, direction, pos_x, pos_y)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(template_row_id)
            .bind(cube_id)
            .bind(direction)
            .bind(pos_x)
            .bind(pos_y)
            .execute(&self.pool)
            .await?;
        }

        tracing::info!(
            "Loaded {} cubes from talent scheme {} into template {}",
            cubes.len(),
            talent_id,
            template_id
        );

        if template_id == hero_data.record.use_talent_template_id {
            sqlx::query("DELETE FROM hero_talent_cubes WHERE hero_uid = ?")
                .bind(hero_data.record.uid)
                .execute(&self.pool)
                .await?;

            for (cube_id, direction, pos_x, pos_y) in &cubes {
                sqlx::query(
                    "INSERT INTO hero_talent_cubes
                     (hero_uid, cube_id, direction, pos_x, pos_y)
                     VALUES (?, ?, ?, ?, ?)",
                )
                .bind(hero_data.record.uid)
                .bind(cube_id)
                .bind(direction)
                .bind(pos_x)
                .bind(pos_y)
                .execute(&self.pool)
                .await?;
            }

            tracing::info!(
                "Updated active talent cubes for hero {} (template {})",
                hero_id,
                template_id
            );
        }

        let template_data: (String, i32) =
            sqlx::query_as("SELECT name, style FROM hero_talent_templates WHERE id = ?")
                .bind(template_row_id)
                .fetch_one(&self.pool)
                .await?;

        let talent_cube_infos: Vec<sonettobuf::TalentCubeInfo> = cubes
            .into_iter()
            .map(
                |(cube_id, direction, pos_x, pos_y)| sonettobuf::TalentCubeInfo {
                    cube_id: Some(cube_id),
                    direction: Some(direction),
                    pos_x: Some(pos_x),
                    pos_y: Some(pos_y),
                },
            )
            .collect();

        Ok(sonettobuf::TalentTemplateInfo {
            id: Some(template_id),
            talent_cube_infos,
            name: Some(template_data.0),
            style: Some(template_data.1),
        })
    }

    pub(super) async fn has_talent_style_impl(&self, hero_id: i32, style: i32) -> Result<bool> {
        let hero_data = self.get(hero_id).await?;

        let has_style: Option<i32> = sqlx::query_scalar(
            "SELECT 1 FROM hero_talent_styles WHERE hero_uid = ? AND style_id = ?",
        )
        .bind(hero_data.record.uid)
        .bind(style)
        .fetch_optional(&self.pool)
        .await?;

        Ok(has_style.is_some())
    }

    pub(super) async fn unlock_talent_style_impl(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        hero_id: i32,
        style: i32,
    ) -> Result<bool> {
        let inserted = sqlx::query(
            "INSERT OR IGNORE INTO hero_talent_styles (hero_uid, style_id)
             SELECT uid, ? FROM heroes WHERE user_id = ? AND hero_id = ?",
        )
        .bind(style)
        .bind(self.user_id)
        .bind(hero_id)
        .execute(&mut **tx)
        .await?;
        if inserted.rows_affected() != 1 {
            return Ok(false);
        }

        let style_bit = 1_i32
            .checked_shl(style.try_into()?)
            .ok_or_else(|| anyhow!("invalid talent style {style}"))?;
        let updated = sqlx::query(
            "UPDATE heroes SET talent_style_unlock = talent_style_unlock | ?
             WHERE user_id = ? AND hero_id = ?",
        )
        .bind(style_bit)
        .bind(self.user_id)
        .bind(hero_id)
        .execute(&mut **tx)
        .await?;

        Ok(updated.rows_affected() == 1)
    }

    pub(super) async fn apply_talent_style_impl(
        &self,
        hero_id: i32,
        template_id: i32,
        style: i32,
    ) -> Result<()> {
        let hero_data = self.get(hero_id).await?;

        if style != 0 {
            let has_style: Option<i32> = sqlx::query_scalar(
                "SELECT 1 FROM hero_talent_styles WHERE hero_uid = ? AND style_id = ?",
            )
            .bind(hero_data.record.uid)
            .bind(style)
            .fetch_optional(&self.pool)
            .await?;

            if has_style.is_none() {
                tracing::warn!(
                    "User {} does not own style {} for hero {}",
                    self.user_id,
                    style,
                    hero_id
                );
                return Err(anyhow::anyhow!("Style not owned"));
            }
        }

        let template_row_id: i64 = sqlx::query_scalar(
            "SELECT id FROM hero_talent_templates WHERE hero_uid = ? AND template_id = ?",
        )
        .bind(hero_data.record.uid)
        .bind(template_id)
        .fetch_one(&self.pool)
        .await?;

        sqlx::query("UPDATE hero_talent_templates SET style = ? WHERE id = ?")
            .bind(style)
            .bind(template_row_id)
            .execute(&self.pool)
            .await?;

        tracing::info!(
            "User {} applied style {} to template {} for hero {}",
            self.user_id,
            style,
            template_id,
            hero_id
        );

        Ok(())
    }

    pub(super) async fn switch_talent_template_impl(
        &self,
        hero_id: i32,
        template_id: i32,
    ) -> Result<sonettobuf::TalentTemplateInfo> {
        let hero_data = self.get(hero_id).await?;

        let template_row_id: i64 = sqlx::query_scalar(
            "SELECT id FROM hero_talent_templates WHERE hero_uid = ? AND template_id = ?",
        )
        .bind(hero_data.record.uid)
        .bind(template_id)
        .fetch_one(&self.pool)
        .await?;

        let cubes: Vec<(i32, i32, i32, i32)> = sqlx::query_as(
            "SELECT cube_id, direction, pos_x, pos_y
             FROM hero_talent_template_cubes
             WHERE template_row_id = ?",
        )
        .bind(template_row_id)
        .fetch_all(&self.pool)
        .await?;

        let template_data: (String, i32) =
            sqlx::query_as("SELECT name, style FROM hero_talent_templates WHERE id = ?")
                .bind(template_row_id)
                .fetch_one(&self.pool)
                .await?;

        sqlx::query("DELETE FROM hero_talent_cubes WHERE hero_uid = ?")
            .bind(hero_data.record.uid)
            .execute(&self.pool)
            .await?;

        for (cube_id, direction, pos_x, pos_y) in &cubes {
            sqlx::query(
                "INSERT INTO hero_talent_cubes
                 (hero_uid, cube_id, direction, pos_x, pos_y)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(hero_data.record.uid)
            .bind(cube_id)
            .bind(direction)
            .bind(pos_x)
            .bind(pos_y)
            .execute(&self.pool)
            .await?;
        }

        sqlx::query("UPDATE heroes SET use_talent_template_id = ? WHERE uid = ? AND user_id = ?")
            .bind(template_id)
            .bind(hero_data.record.uid)
            .bind(self.user_id)
            .execute(&self.pool)
            .await?;

        tracing::info!(
            "User {} switched to talent template {} for hero {}",
            self.user_id,
            template_id,
            hero_id
        );

        let talent_cube_infos = cubes
            .into_iter()
            .map(
                |(cube_id, direction, pos_x, pos_y)| sonettobuf::TalentCubeInfo {
                    cube_id: Some(cube_id),
                    direction: Some(direction),
                    pos_x: Some(pos_x),
                    pos_y: Some(pos_y),
                },
            )
            .collect();

        Ok(sonettobuf::TalentTemplateInfo {
            id: Some(template_id),
            talent_cube_infos,
            name: Some(template_data.0),
            style: Some(template_data.1),
        })
    }
}
