use super::ProfileManager;
use crate::{error::AppError, reward};
use database::{
    db::game::{critters, items, player_card, player_infos},
    models::game::{heros::UserHeroModel, player_card::PlayerCardInfo},
};
use sonettobuf::{
    GetOtherPlayerCardInfoReply, GetPlayerCardInfoReply, SetPlayerCardBadgeReply,
    SetPlayerCardBaseSettingReply, SetPlayerCardCritterReply, SetPlayerCardHeroCoverReply,
    SetPlayerCardProgressSettingReply, SetPlayerCardShowSettingReply, SetPlayerCardThemeReply,
};
use sqlx::SqlitePool;
use std::{collections::HashSet, ops::RangeInclusive};

const PLAYER_CARD_THEME_ITEM_SUB_TYPE: i32 = 21;

impl ProfileManager {
    pub async fn card_info(&self, db: &SqlitePool) -> Result<GetPlayerCardInfoReply, AppError> {
        let info = player_card::get_player_card_info(db, self.player_id).await?;

        Ok(GetPlayerCardInfoReply {
            player_card_info: Some(info.into()),
        })
    }

    pub async fn other_card_info(
        &self,
        db: &SqlitePool,
        target_id: i64,
    ) -> Result<GetOtherPlayerCardInfoReply, AppError> {
        let player_info = player_infos::get_player_info_data(db, target_id)
            .await?
            .ok_or(AppError::InvalidRequest)?;
        let card = player_card::get_player_card_info(db, target_id).await?;

        Ok(GetOtherPlayerCardInfoReply {
            player_info: Some(player_info.into()),
            player_card_info: Some(card.into()),
        })
    }

    pub async fn set_card_show_settings(
        &self,
        db: &SqlitePool,
        show_settings: Vec<String>,
    ) -> Result<SetPlayerCardShowSettingReply, AppError> {
        if !valid_show_settings(&show_settings) {
            return Err(AppError::InvalidRequest);
        }
        let encoded = serde_json::to_string(&show_settings)?;
        update(db, self.player_id, |card| card.show_settings = encoded).await?;
        Ok(SetPlayerCardShowSettingReply { show_settings })
    }

    pub async fn set_card_progress_setting(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
        setting: String,
    ) -> Result<SetPlayerCardProgressSettingReply, AppError> {
        if !valid_layout(&setting, 1..=5, |id| {
            tables.player_newspaper.get(id).is_some()
        }) {
            return Err(AppError::InvalidRequest);
        }
        update(db, self.player_id, |card| {
            card.progress_setting = setting.clone()
        })
        .await?;
        Ok(SetPlayerCardProgressSettingReply {
            progress_setting: Some(setting),
        })
    }

    pub async fn set_card_base_setting(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
        setting: String,
    ) -> Result<SetPlayerCardBaseSettingReply, AppError> {
        if !valid_layout(&setting, 2..=4, |id| tables.playercard.get(id).is_some()) {
            return Err(AppError::InvalidRequest);
        }
        update(db, self.player_id, |card| {
            card.base_setting = setting.clone()
        })
        .await?;
        Ok(SetPlayerCardBaseSettingReply {
            base_setting: Some(setting),
        })
    }

    pub async fn set_card_hero_cover(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
        hero_cover: String,
    ) -> Result<SetPlayerCardHeroCoverReply, AppError> {
        let mut fields = hero_cover.split('#');
        let (Some(hero_id), Some(skin_id), Some(l2d), None) = (
            fields.next().and_then(|value| value.parse::<i32>().ok()),
            fields.next().and_then(|value| value.parse::<i32>().ok()),
            fields.next().and_then(|value| value.parse::<i32>().ok()),
            fields.next(),
        ) else {
            return Err(AppError::InvalidRequest);
        };
        let heroes = UserHeroModel::new(self.player_id, db.clone());
        let skin_matches = tables
            .skin
            .get(skin_id)
            .is_some_and(|skin| skin.character_id == hero_id);
        if !matches!(l2d, 0 | 1)
            || !skin_matches
            || !heroes.has_hero(hero_id).await?
            || !heroes.has_skin(skin_id).await?
        {
            return Err(AppError::InvalidRequest);
        }

        update(db, self.player_id, |card| {
            card.hero_cover = hero_cover.clone()
        })
        .await?;
        Ok(SetPlayerCardHeroCoverReply {
            hero_cover: Some(hero_cover),
        })
    }

    pub async fn set_card_theme(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
        theme_id: i32,
    ) -> Result<SetPlayerCardThemeReply, AppError> {
        if theme_id != 0 {
            if tables
                .item
                .get(theme_id)
                .is_none_or(|item| item.sub_type != PLAYER_CARD_THEME_ITEM_SUB_TYPE)
            {
                return Err(AppError::InvalidRequest);
            }
            if items::get_item(db, self.player_id, theme_id as u32)
                .await?
                .is_none_or(|item| item.quantity <= 0)
            {
                return Err(AppError::InvalidRequest);
            }
        }
        update(db, self.player_id, |card| card.theme_id = theme_id).await?;
        Ok(SetPlayerCardThemeReply {
            theme_id: Some(theme_id),
        })
    }

    pub async fn set_card_critter(
        &self,
        db: &SqlitePool,
        critter_uid: i64,
    ) -> Result<SetPlayerCardCritterReply, AppError> {
        let critter = critters::get_player_critter(db, self.player_id, critter_uid)
            .await?
            .ok_or(AppError::InvalidRequest)?;
        let encoded = format!(
            "{}#{}#{}",
            critter.uid,
            critter.define_id,
            i32::from(critter.special_skin)
        );
        update(db, self.player_id, |card| card.critter = encoded).await?;
        Ok(SetPlayerCardCritterReply {
            critter_uid: Some(critter_uid as i32),
        })
    }

    pub async fn set_card_badges(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
        badge_ids: Vec<i32>,
    ) -> Result<SetPlayerCardBadgeReply, AppError> {
        if badge_ids.len() > 3 || badge_ids.iter().collect::<HashSet<_>>().len() != badge_ids.len()
        {
            return Err(AppError::InvalidRequest);
        }
        for badge_id in &badge_ids {
            let badge = tables
                .playercard_badge
                .get(*badge_id)
                .ok_or(AppError::InvalidRequest)?;
            let task = tables
                .activity229_task
                .get(badge.task_id)
                .ok_or(AppError::InvalidRequest)?;
            let unlock_items = reward::parse(&task.bonus).items;
            if unlock_items.is_empty() {
                return Err(AppError::InvalidRequest);
            }
            for (item_id, _) in unlock_items {
                if items::get_item(db, self.player_id, item_id)
                    .await?
                    .is_none_or(|item| item.quantity <= 0)
                {
                    return Err(AppError::InvalidRequest);
                }
            }
        }

        let encoded = serde_json::to_string(&badge_ids)?;
        update(db, self.player_id, |card| card.badge_ids = encoded).await?;
        Ok(SetPlayerCardBadgeReply { badge_ids })
    }

    pub async fn increment_hero_cover_times(
        &self,
        db: &SqlitePool,
        count: i32,
    ) -> Result<(), AppError> {
        player_card::increment_hero_cover_times(db, self.player_id, count).await?;
        Ok(())
    }
}

async fn update(
    db: &SqlitePool,
    player_id: i64,
    change: impl FnOnce(&mut PlayerCardInfo),
) -> Result<(), AppError> {
    let mut card = player_card::get_player_card_info(db, player_id).await?;
    change(&mut card);
    player_card::update_player_card_info(db, &card).await?;
    Ok(())
}

fn valid_show_settings(settings: &[String]) -> bool {
    let mut keys = HashSet::new();
    settings.iter().all(|setting| {
        let Some((key, value)) = setting.split_once('#') else {
            return false;
        };
        let Ok(key) = key.parse::<i32>() else {
            return false;
        };
        let Ok(value) = value.parse::<i32>() else {
            return false;
        };
        keys.insert(key) && matches!((key, value), (1, 1 | 2) | (2, 0 | 1))
    })
}

fn valid_layout(
    setting: &str,
    positions: RangeInclusive<i32>,
    valid_id: impl Fn(i32) -> bool,
) -> bool {
    if setting.is_empty() {
        return true;
    }
    let mut used_positions = HashSet::new();
    let mut used_ids = HashSet::new();
    setting.split('|').all(|entry| {
        let Some((position, id)) = entry.split_once('#') else {
            return false;
        };
        let (Ok(position), Ok(id)) = (position.parse::<i32>(), id.parse::<i32>()) else {
            return false;
        };
        positions.contains(&position)
            && used_positions.insert(position)
            && used_ids.insert(id)
            && valid_id(id)
    })
}

#[cfg(test)]
mod tests;
