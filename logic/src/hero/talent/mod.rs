use super::HeroManager;
use crate::{error::AppError, reward};
use database::models::game::heros::{HeroModel, UserHeroModel};
use sonettobuf::{
    HeroInfo, HeroTalentStylePercent, HeroTalentStyleStatReply, HeroTalentUpReply,
    PutTalentCubeBatchReply, PutTalentCubeReply, PutTalentSchemeReply, RenameTalentTemplateReply,
    TakeoffAllTalentCubeReply, TalentStyleReadReply, UnlockTalentStyleReply, UseTalentStyleReply,
    UseTalentTemplateReply,
};
use sqlx::SqlitePool;

impl HeroManager {
    pub async fn talent_style_read(
        &self,
        db: &SqlitePool,
        hero_id: i32,
    ) -> Result<(TalentStyleReadReply, HeroInfo), AppError> {
        style_read(db, self.player_id, hero_id).await
    }

    pub async fn talent_up(
        &self,
        db: &SqlitePool,
        hero_id: i32,
    ) -> Result<(HeroTalentUpReply, HeroInfo, reward::ConsumedRewards), AppError> {
        talent_up(db, self.player_id, hero_id).await
    }

    pub async fn put_talent_cube(
        &self,
        db: &SqlitePool,
        hero_id: i32,
        template_id: i32,
        get_cube: Option<(i32, i32)>,
        placed_cube: Option<(i32, i32, i32, i32)>,
    ) -> Result<(PutTalentCubeReply, HeroInfo), AppError> {
        put_cube(
            db,
            self.player_id,
            hero_id,
            template_id,
            get_cube,
            placed_cube,
        )
        .await
    }

    pub async fn put_talent_cube_batch(
        &self,
        db: &SqlitePool,
        hero_id: i32,
        template_id: i32,
        style: Option<i32>,
        cubes: Vec<(i32, i32, i32, i32)>,
    ) -> Result<(PutTalentCubeBatchReply, HeroInfo), AppError> {
        put_cube_batch(db, self.player_id, hero_id, template_id, style, cubes).await
    }

    pub async fn takeoff_all_talent_cubes(
        &self,
        db: &SqlitePool,
        hero_id: i32,
        template_id: i32,
    ) -> Result<(TakeoffAllTalentCubeReply, HeroInfo), AppError> {
        takeoff_all(db, self.player_id, hero_id, template_id).await
    }

    pub async fn rename_talent_template(
        &self,
        db: &SqlitePool,
        hero_id: i32,
        template_id: i32,
        name: String,
    ) -> Result<RenameTalentTemplateReply, AppError> {
        rename_template(db, self.player_id, hero_id, template_id, name).await
    }

    pub async fn put_talent_scheme(
        &self,
        db: &SqlitePool,
        hero_id: i32,
        talent_id: i32,
        talent_mould: i32,
        template_id: i32,
    ) -> Result<(PutTalentSchemeReply, HeroInfo), AppError> {
        put_scheme(
            db,
            self.player_id,
            hero_id,
            talent_id,
            talent_mould,
            template_id,
        )
        .await
    }

    pub fn talent_style_stat(&self, hero_id: i32) -> HeroTalentStyleStatReply {
        style_stat(hero_id)
    }

    pub async fn unlock_talent_style(
        &self,
        db: &SqlitePool,
        hero_id: i32,
        style: i32,
    ) -> Result<(UnlockTalentStyleReply, HeroInfo, reward::ConsumedRewards), AppError> {
        unlock_style(db, self.player_id, hero_id, style).await
    }

    pub async fn use_talent_style(
        &self,
        db: &SqlitePool,
        hero_id: i32,
        template_id: i32,
        style: i32,
    ) -> Result<(UseTalentStyleReply, HeroInfo), AppError> {
        use_style(db, self.player_id, hero_id, template_id, style).await
    }

    pub async fn use_talent_template(
        &self,
        db: &SqlitePool,
        hero_id: i32,
        template_id: i32,
    ) -> Result<(UseTalentTemplateReply, HeroInfo), AppError> {
        use_template(db, self.player_id, hero_id, template_id).await
    }
}

async fn style_read(
    db: &SqlitePool,
    player_id: i64,
    hero_id: i32,
) -> Result<(TalentStyleReadReply, HeroInfo), AppError> {
    let hero = UserHeroModel::new(player_id, db.clone());
    hero.talent_style_read(hero_id).await?;
    let updated = super::snapshot(db, hero.get(hero_id).await?).await?;

    Ok((
        TalentStyleReadReply {
            hero_id: Some(hero_id),
        },
        updated,
    ))
}

async fn talent_up(
    db: &SqlitePool,
    player_id: i64,
    hero_id: i32,
) -> Result<(HeroTalentUpReply, HeroInfo, reward::ConsumedRewards), AppError> {
    let hero = UserHeroModel::new(player_id, db.clone());
    let current = hero.get(hero_id).await?;
    let next_talent = current
        .record
        .talent
        .checked_add(1)
        .ok_or(AppError::InvalidRequest)?;
    let talent = config::configs::get()
        .character_talent(hero_id, next_talent)
        .ok_or(AppError::InvalidRequest)?;
    if current.record.rank < talent.requirement {
        return Err(AppError::InvalidRequest);
    }

    let mut tx = db.begin().await?;
    let consumed = reward::consume(&mut tx, player_id, &reward::parse(&talent.consume)).await?;
    if !hero
        .update_talent(&mut tx, hero_id, current.record.talent, next_talent)
        .await?
    {
        return Err(AppError::InvalidRequest);
    }
    tx.commit().await?;
    let updated = super::snapshot(db, hero.get(hero_id).await?).await?;

    Ok((
        HeroTalentUpReply {
            hero_id: Some(hero_id),
            talent_id: Some(next_talent),
        },
        updated,
        consumed,
    ))
}

async fn put_cube(
    db: &SqlitePool,
    player_id: i64,
    hero_id: i32,
    template_id: i32,
    get_cube: Option<(i32, i32)>,
    put_cube: Option<(i32, i32, i32, i32)>,
) -> Result<(PutTalentCubeReply, HeroInfo), AppError> {
    let hero = UserHeroModel::new(player_id, db.clone());

    if let Some((pos_x, pos_y)) = get_cube {
        hero.remove_talent_cube(hero_id, template_id, pos_x, pos_y)
            .await?;
    }

    if let Some((cube_id, direction, pos_x, pos_y)) = put_cube {
        hero.place_talent_cube(hero_id, template_id, cube_id, direction, pos_x, pos_y)
            .await?;
    }

    hero.sync_active_talent_cubes(hero_id, template_id, get_cube, put_cube)
        .await?;
    let template_info = hero.get_template_info(hero_id, template_id).await?;
    let updated = super::snapshot(db, hero.get(hero_id).await?).await?;

    Ok((
        PutTalentCubeReply {
            hero_id: Some(hero_id),
            template_info: Some(template_info),
        },
        updated,
    ))
}

async fn put_cube_batch(
    db: &SqlitePool,
    player_id: i64,
    hero_id: i32,
    template_id: i32,
    style: Option<i32>,
    cubes: Vec<(i32, i32, i32, i32)>,
) -> Result<(PutTalentCubeBatchReply, HeroInfo), AppError> {
    let hero = UserHeroModel::new(player_id, db.clone());
    hero.replace_talent_cubes(hero_id, template_id, cubes)
        .await?;

    if let Some(style) = style {
        hero.apply_talent_style(hero_id, template_id, style).await?;
    }

    let template_info = hero.get_template_info(hero_id, template_id).await?;
    let updated = super::snapshot(db, hero.get(hero_id).await?).await?;

    Ok((
        PutTalentCubeBatchReply {
            hero_id: Some(hero_id),
            style,
            template_info: Some(template_info),
        },
        updated,
    ))
}

async fn takeoff_all(
    db: &SqlitePool,
    player_id: i64,
    hero_id: i32,
    template_id: i32,
) -> Result<(TakeoffAllTalentCubeReply, HeroInfo), AppError> {
    let hero = UserHeroModel::new(player_id, db.clone());
    let template_info = hero
        .replace_talent_cubes(hero_id, template_id, Vec::new())
        .await?;
    let updated = super::snapshot(db, hero.get(hero_id).await?).await?;

    Ok((
        TakeoffAllTalentCubeReply {
            hero_id: Some(hero_id),
            template_info: Some(template_info),
        },
        updated,
    ))
}

async fn rename_template(
    db: &SqlitePool,
    player_id: i64,
    hero_id: i32,
    template_id: i32,
    name: String,
) -> Result<RenameTalentTemplateReply, AppError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 10 {
        return Err(AppError::InvalidRequest);
    }
    let template_info = UserHeroModel::new(player_id, db.clone())
        .rename_talent_template(hero_id, template_id, name)
        .await
        .map_err(|_| AppError::InvalidRequest)?;

    Ok(RenameTalentTemplateReply {
        hero_id: Some(hero_id),
        template_info: Some(template_info),
    })
}

async fn put_scheme(
    db: &SqlitePool,
    player_id: i64,
    hero_id: i32,
    talent_id: i32,
    talent_mould: i32,
    template_id: i32,
) -> Result<(PutTalentSchemeReply, HeroInfo), AppError> {
    let hero = UserHeroModel::new(player_id, db.clone());
    let template_info = hero
        .load_talent_scheme(hero_id, talent_id, talent_mould, template_id)
        .await?;
    let updated = super::snapshot(db, hero.get(hero_id).await?).await?;

    Ok((
        PutTalentSchemeReply {
            hero_id: Some(hero_id),
            template_info: Some(template_info),
        },
        updated,
    ))
}

fn style_stat(hero_id: i32) -> HeroTalentStyleStatReply {
    let style_percent_list = config::configs::get()
        .talent_style_cost
        .iter()
        .filter(|row| row.hero_id == hero_id)
        .map(|row| HeroTalentStylePercent {
            style: Some(row.style_id),
            percent: Some(0),
        })
        .collect();

    HeroTalentStyleStatReply {
        hero_id: Some(hero_id),
        style_percent_list,
    }
}

async fn unlock_style(
    db: &SqlitePool,
    player_id: i64,
    hero_id: i32,
    style: i32,
) -> Result<(UnlockTalentStyleReply, HeroInfo, reward::ConsumedRewards), AppError> {
    let hero = UserHeroModel::new(player_id, db.clone());
    let cost = config::configs::get()
        .talent_style_cost(hero_id, style)
        .ok_or(AppError::InvalidRequest)?;
    let consumed = if hero.has_talent_style(hero_id, style).await? {
        reward::ConsumedRewards::default()
    } else {
        let mut tx = db.begin().await?;
        let consumed = reward::consume(&mut tx, player_id, &reward::parse(&cost.consume)).await?;
        if !hero.unlock_talent_style(&mut tx, hero_id, style).await? {
            return Err(AppError::InvalidRequest);
        }
        tx.commit().await?;
        consumed
    };
    let updated = super::snapshot(db, hero.get(hero_id).await?).await?;

    Ok((
        UnlockTalentStyleReply {
            hero_id: Some(hero_id),
            style: Some(style),
        },
        updated,
        consumed,
    ))
}

async fn use_style(
    db: &SqlitePool,
    player_id: i64,
    hero_id: i32,
    template_id: i32,
    style: i32,
) -> Result<(UseTalentStyleReply, HeroInfo), AppError> {
    let hero = UserHeroModel::new(player_id, db.clone());
    hero.apply_talent_style(hero_id, template_id, style).await?;
    let updated = super::snapshot(db, hero.get(hero_id).await?).await?;

    Ok((
        UseTalentStyleReply {
            hero_id: Some(hero_id),
            template_id: Some(template_id),
            style: Some(style),
        },
        updated,
    ))
}

async fn use_template(
    db: &SqlitePool,
    player_id: i64,
    hero_id: i32,
    template_id: i32,
) -> Result<(UseTalentTemplateReply, HeroInfo), AppError> {
    let hero = UserHeroModel::new(player_id, db.clone());
    let template_info = hero.switch_talent_template(hero_id, template_id).await?;
    let updated = super::snapshot(db, hero.get(hero_id).await?).await?;

    Ok((
        UseTalentTemplateReply {
            hero_id: Some(hero_id),
            template_info: Some(template_info),
        },
        updated,
    ))
}

#[cfg(test)]
mod test;
