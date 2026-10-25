use super::ProfileManager;
use crate::{error::AppError, reward};
use chrono::{Datelike, NaiveDate};
use database::db::{
    game::{currencies, items, open_infos, player_card, player_infos},
    user::account,
};
use sonettobuf::{
    GetOpenInfoReply, GetOtherPlayerInfoReply, GetPlayerInfoReply, HeroBirthdayInfo,
    HeroInfoListReply, PlayerInfoPush, RenameReply, SetBirthdayReply, SetCharacterAgeReply,
    SetPlayerBgReply, SetPortraitReply, SetShowHeroUniqueIdsReply, SetSignatureReply,
};
use sqlx::SqlitePool;
use std::collections::HashSet;

const PLAYER_SIGNATURE_LIMIT_CONST_ID: i32 = 9;
const BIRTHDAY_START_YEAR_CONST_ID: i32 = 5000;

impl ProfileManager {
    pub async fn reconcile_open_infos(
        &self,
        db: &SqlitePool,
    ) -> Result<Vec<sonettobuf::OpenInfo>, AppError> {
        Ok(open_infos::reconcile_progression(db, self.player_id).await?)
    }

    pub async fn get_player_info(&self, db: &SqlitePool) -> Result<GetPlayerInfoReply, AppError> {
        let player_info = player_infos::get_player_info_data(db, self.player_id)
            .await?
            .ok_or_else(|| AppError::Custom("Player info not found".to_string()))?;

        let openinfos = open_infos::list_open_infos(db, self.player_id).await?;

        Ok(GetPlayerInfoReply {
            player_info: Some(player_info.into()),
            openinfos,
            can_rename: Some(true),
            main_thumbnail: Some(false),
            ext_rename: Some(0),
        })
    }

    pub async fn get_other_player_info(
        &self,
        db: &SqlitePool,
        target_id: i64,
    ) -> Result<GetOtherPlayerInfoReply, AppError> {
        let player_info = player_infos::get_player_info_data(db, target_id)
            .await?
            .ok_or(AppError::InvalidRequest)?;
        let card = player_card::get_player_card_info(db, target_id).await?;

        Ok(GetOtherPlayerInfoReply {
            player_info: Some(player_info.into()),
            hero_cover: Some(card.hero_cover),
        })
    }

    pub async fn get_open_info(
        &self,
        db: &SqlitePool,
        id: i32,
    ) -> Result<GetOpenInfoReply, AppError> {
        Ok(GetOpenInfoReply {
            open_ifo: open_infos::get_open_info(db, self.player_id, id).await?,
        })
    }

    pub async fn hero_info_list(&self, db: &SqlitePool) -> Result<HeroInfoListReply, AppError> {
        let hero = database::models::game::heros::UserHeroModel::new(self.player_id, db.clone());
        let heroes = hero.get_all_heroes().await?;
        let touch_count = hero.get_touch_count().await?;
        let all_skins = hero.get_skins().await?;
        let birthday_infos = hero.get_birthdays().await?;

        let mut hero_infos = Vec::with_capacity(heroes.len());
        for hero in heroes {
            hero_infos.push(crate::hero::snapshot_data(db, hero).await?);
        }

        Ok(HeroInfoListReply {
            heros: hero_infos,
            touch_count_left: touch_count,
            all_hero_skin: all_skins,
            birthday_infos: birthday_infos
                .into_iter()
                .map(|(hero_id, count)| HeroBirthdayInfo {
                    hero_id: Some(hero_id),
                    birthday_count: Some(count),
                })
                .collect(),
        })
    }

    pub async fn set_portrait(
        &self,
        db: &SqlitePool,
        portrait: i32,
    ) -> Result<SetPortraitReply, AppError> {
        player_infos::set_portrait(db, self.player_id, portrait).await?;
        Ok(SetPortraitReply {})
    }

    pub async fn set_signature(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
        signature: String,
    ) -> Result<SetSignatureReply, AppError> {
        let limit = tables
            .r#const
            .get(PLAYER_SIGNATURE_LIMIT_CONST_ID)
            .and_then(|value| value.value.parse().ok())
            .ok_or(AppError::InvalidRequest)?;
        if signature.chars().count() > limit {
            return Err(AppError::InvalidRequest);
        }
        player_infos::set_signature(db, self.player_id, &signature).await?;
        Ok(SetSignatureReply {})
    }

    pub async fn set_birthday(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
        birthday: String,
    ) -> Result<SetBirthdayReply, AppError> {
        let start_year = tables
            .r#const
            .get(BIRTHDAY_START_YEAR_CONST_ID)
            .and_then(|value| value.value.parse().ok())
            .ok_or(AppError::InvalidRequest)?;
        let date = NaiveDate::parse_from_str(&birthday, "%Y-%m-%d")
            .map_err(|_| AppError::InvalidRequest)?;
        if date.year() < start_year || date > common::time::ServerTime::server_date().date_naive() {
            return Err(AppError::InvalidRequest);
        }
        if !player_infos::set_birthday_once(db, self.player_id, &birthday).await? {
            return Err(AppError::InvalidRequest);
        }
        Ok(SetBirthdayReply {})
    }

    pub async fn set_character_age(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
        character_age: Vec<i32>,
    ) -> Result<SetCharacterAgeReply, AppError> {
        let unique = character_age.iter().copied().collect::<HashSet<_>>();
        if unique.len() != character_age.len()
            || character_age
                .iter()
                .any(|id| tables.handbook_character_age.get(*id).is_none())
        {
            return Err(AppError::InvalidRequest);
        }

        player_infos::set_character_age(db, self.player_id, &character_age).await?;
        Ok(SetCharacterAgeReply { character_age })
    }

    pub async fn set_player_bg(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
        bg_id: i32,
    ) -> Result<SetPlayerBgReply, AppError> {
        let bg = tables
            .player_background_by_item(bg_id)
            .ok_or(AppError::InvalidRequest)?;
        if bg.item != 0
            && items::get_item(db, self.player_id, bg.item as u32)
                .await?
                .is_none_or(|item| item.quantity <= 0)
        {
            return Err(AppError::InvalidRequest);
        }
        player_infos::set_player_bg(db, self.player_id, bg_id).await?;
        Ok(SetPlayerBgReply { bg_id: Some(bg_id) })
    }

    pub async fn set_show_hero_unique_ids(
        &self,
        db: &SqlitePool,
        hero_uids: Vec<i64>,
    ) -> Result<(SetShowHeroUniqueIdsReply, PlayerInfoPush), AppError> {
        player_infos::set_show_hero(db, self.player_id, hero_uids).await?;

        Ok((SetShowHeroUniqueIdsReply {}, self.snapshot(db).await?))
    }

    pub async fn snapshot(&self, db: &SqlitePool) -> Result<PlayerInfoPush, AppError> {
        let player_info = player_infos::get_player_info_data(db, self.player_id)
            .await?
            .ok_or_else(|| AppError::Custom("Player info not found".to_string()))?;

        Ok(PlayerInfoPush {
            player_info: Some(player_info.into()),
        })
    }
}

pub fn level_up_rewards(change: player_infos::PlayerLevelChange) -> reward::RewardSet {
    let mut rewards = reward::RewardSet::default();
    for level in config::configs::get().player_levels_between(change.from, change.to) {
        rewards.extend(reward::parse_bonus(level.bonus));
        rewards
            .currencies
            .push((currencies::POWER_CURRENCY_ID, level.add_up_recover_power));
    }
    rewards
}

impl ProfileManager {
    pub async fn rename(
        &self,
        db: &SqlitePool,
        name: String,
        guide_id: i32,
        step_id: i32,
    ) -> Result<(RenameReply, PlayerInfoPush), AppError> {
        account::rename_user_and_update_guide(db, self.player_id, name, guide_id, step_id).await?;

        Ok((
            RenameReply {
                can_rename: Some(true),
                ext_rename: Some(0),
            },
            self.snapshot(db).await?,
        ))
    }
}

#[cfg(test)]
mod tests;
