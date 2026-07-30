use super::*;
use chrono::Datelike;
use database::db::game::{equipment, sign_in};

pub(crate) async fn snapshot_data(db: &SqlitePool, hero: HeroData) -> Result<HeroInfo, AppError> {
    Ok(battle::engine::entity::stats::hero_info(db, hero).await?)
}

impl HeroManager {
    pub async fn snapshots(
        self,
        db: &SqlitePool,
        hero_ids: impl IntoIterator<Item = i32>,
    ) -> Result<Vec<HeroInfo>, AppError> {
        let heroes = UserHeroModel::new(self.player_id, db.clone());
        let mut snapshots = Vec::new();
        for hero_id in hero_ids {
            snapshots.push(snapshot_data(db, heroes.get_hero(hero_id).await?).await?);
        }
        Ok(snapshots)
    }

    pub async fn mark_favor(
        self,
        db: &SqlitePool,
        hero_id: i32,
        is_favor: bool,
    ) -> Result<MarkHeroFavorReply, AppError> {
        UserHeroModel::new(self.player_id, db.clone())
            .set_favor(hero_id, is_favor)
            .await?;

        Ok(MarkHeroFavorReply {
            hero_id: Some(hero_id),
            is_favor: Some(is_favor),
        })
    }

    pub async fn unmark_new(
        self,
        db: &SqlitePool,
        hero_id: i32,
    ) -> Result<UnMarkIsNewReply, AppError> {
        UserHeroModel::new(self.player_id, db.clone())
            .unmark_new(hero_id)
            .await?;

        Ok(UnMarkIsNewReply {
            hero_id: Some(hero_id),
        })
    }

    pub async fn unlock_voice(
        self,
        db: &SqlitePool,
        hero_id: i32,
        voice_id: i32,
    ) -> Result<UnlockVoiceReply, AppError> {
        if hero_id <= 0 || voice_id <= 0 {
            return Err(AppError::InvalidRequest);
        }
        UserHeroModel::new(self.player_id, db.clone())
            .unlock_voice(hero_id, voice_id)
            .await
            .map_err(|_| AppError::InvalidRequest)?;

        Ok(UnlockVoiceReply {
            hero_id: Some(hero_id),
            voice_id: Some(voice_id),
        })
    }

    pub async fn unlock_item(
        self,
        db: &SqlitePool,
        hero_id: i32,
        item_id: i32,
    ) -> Result<(ItemUnlockReply, (i32, i32)), AppError> {
        let tables = config::configs::get();
        let item = tables
            .character_unlock_item(hero_id, item_id)
            .ok_or(AppError::InvalidRequest)?;
        let heroes = UserHeroModel::new(self.player_id, db.clone());
        let hero = heroes
            .get_hero(hero_id)
            .await
            .map_err(|_| AppError::InvalidRequest)?;
        let mut condition = item.unlock_conditine.split('#');
        let condition_type = condition
            .next()
            .and_then(|value| value.parse::<i32>().ok())
            .ok_or(AppError::InvalidRequest)?;
        let required = condition
            .next()
            .and_then(|value| value.parse::<i32>().ok())
            .ok_or(AppError::InvalidRequest)?;
        let actual = match condition_type {
            1 => tables.faith_percent(hero.record.faith),
            2 => hero.record.rank,
            3 => hero.record.level,
            4 => hero.record.ex_skill_level,
            5 => hero.record.talent,
            6 => {
                player_infos::get_player_info_data(db, self.player_id)
                    .await?
                    .ok_or(AppError::InvalidRequest)?
                    .player_info
                    .last_episode_id
            }
            _ => return Err(AppError::InvalidRequest),
        };
        if actual < required {
            return Err(AppError::InvalidRequest);
        }

        let mut reward = item.unlock_rewards.split('#');
        if reward.next() != Some("2") {
            return Err(AppError::InvalidRequest);
        }
        let currency_id = reward
            .next()
            .and_then(|value| value.parse::<i32>().ok())
            .ok_or(AppError::InvalidRequest)?;
        let amount = reward
            .next()
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|amount| *amount > 0)
            .ok_or(AppError::InvalidRequest)?;
        let limit = tables
            .currency
            .get(currency_id)
            .ok_or(AppError::InvalidRequest)?
            .max_limit;

        if !heroes
            .unlock_item_with_currency_reward(hero.record.uid, item_id, currency_id, amount, limit)
            .await?
        {
            return Err(AppError::InvalidRequest);
        }

        Ok((
            ItemUnlockReply {
                hero_id: Some(hero_id),
                item_id: Some(item_id),
            },
            (currency_id, amount),
        ))
    }

    pub async fn use_skin(
        self,
        db: &SqlitePool,
        hero_id: i32,
        skin_id: i32,
    ) -> Result<UseSkinReply, AppError> {
        if !UserHeroModel::new(self.player_id, db.clone())
            .update_skin(hero_id, skin_id)
            .await?
        {
            return Err(AppError::InvalidRequest);
        }

        Ok(UseSkinReply {
            hero_id: Some(hero_id),
            skin_id: Some(skin_id),
        })
    }

    pub async fn read_red_dot(
        self,
        db: &SqlitePool,
        hero_id: i32,
        red_dot: i32,
    ) -> Result<HeroRedDotReadReply, AppError> {
        UserHeroModel::new(self.player_id, db.clone())
            .read_red_dot(hero_id, red_dot)
            .await?;

        Ok(HeroRedDotReadReply {
            hero_id: Some(hero_id),
            red_dot: Some(red_dot),
        })
    }

    pub async fn touch(self, db: &SqlitePool, hero_id: i32) -> Result<HeroTouchReply, AppError> {
        let tables = config::configs::get();
        let faith_amount = tables
            .r#const
            .get(HeroConstId::TouchFaith as i32)
            .and_then(|row| row.value.parse().ok())
            .ok_or(AppError::InvalidRequest)?;
        let max_faith = tables.max_faith();
        let hero = UserHeroModel::new(self.player_id, db.clone());
        let touch_count_left = hero.use_touch(hero_id, faith_amount, max_faith).await?;

        Ok(HeroTouchReply {
            touch_count_left: Some(touch_count_left.unwrap_or(0)),
            success: Some(touch_count_left.is_some()),
        })
    }

    pub async fn gain_battle_faith_in_transaction(
        self,
        tx: &mut Transaction<'_, Sqlite>,
        fight_group: &sonettobuf::FightGroup,
        amount: i32,
    ) -> Result<Vec<i32>, AppError> {
        let max_faith = config::configs::get().max_faith();
        let hero_uids = fight_group
            .hero_list
            .iter()
            .chain(&fight_group.sub_hero_list)
            .copied()
            .collect::<Vec<_>>();

        Ok(UserHeroModel::add_faith_by_uids_in_transaction(
            tx,
            self.player_id,
            &hero_uids,
            amount,
            max_faith,
        )
        .await?)
    }

    pub async fn default_equip(
        self,
        db: &SqlitePool,
        hero_id: i32,
        equip_uid: i64,
    ) -> Result<(HeroDefaultEquipReply, HeroInfo), AppError> {
        if equip_uid != 0 {
            let owned = equipment::get_equipment_by_uid(db, self.player_id, equip_uid)
                .await
                .map_err(|_| AppError::InvalidRequest)?;
            let tables = config::configs::get();
            let equip = tables
                .equip
                .get(owned.equip_id)
                .ok_or(AppError::InvalidRequest)?;
            if !tables.is_normal_equipment(equip) {
                return Err(AppError::InvalidRequest);
            }
        }
        let hero = UserHeroModel::new(self.player_id, db.clone());
        if !hero.update_equipped_gear(hero_id, equip_uid).await? {
            return Err(AppError::InvalidRequest);
        }
        let updated = snapshot_data(db, hero.get_hero(hero_id).await?).await?;

        Ok((
            HeroDefaultEquipReply {
                hero_id: Some(hero_id),
                default_equip_uid: Some(equip_uid),
            },
            updated,
        ))
    }

    pub async fn birthday(
        self,
        db: &SqlitePool,
        hero_id: i32,
    ) -> Result<reward::RewardedReply<GetHeroBirthdayReply>, AppError> {
        let character = config::configs::get()
            .character
            .get(hero_id)
            .ok_or(AppError::InvalidRequest)?;
        if character.is_online != "1" || character.is_sp {
            return Err(AppError::InvalidRequest);
        }
        let now = common::time::ServerTime::server_date();
        let (birthday_count, last_claim_year) =
            sign_in::get_hero_birthday_claim(db, self.player_id, hero_id)
                .await?
                .unwrap_or_default();
        if last_claim_year == now.year() {
            return Err(AppError::InvalidRequest);
        }
        let rewards = birthday_reward(character, birthday_count, now.month(), now.day())
            .ok_or(AppError::InvalidRequest)?;
        let material_changes = rewards.material_changes();
        let mut tx = db.begin().await?;
        if !sign_in::claim_hero_birthday_in_transaction(
            &mut tx,
            self.player_id,
            hero_id,
            birthday_count,
            now.year(),
        )
        .await?
        {
            return Err(AppError::InvalidRequest);
        }
        let rewards = reward::apply_in_transaction(&mut tx, db, self.player_id, rewards).await?;
        tx.commit().await?;

        Ok(reward::RewardedReply {
            reply: GetHeroBirthdayReply {
                hero_id: Some(hero_id),
            },
            rewards,
            material_changes,
        })
    }
}

pub(super) fn birthday_reward(
    character: &config::character::Character,
    birthday_count: i32,
    month: u32,
    day: u32,
) -> Option<reward::RewardSet> {
    let (birthday_month, birthday_day) = character.role_birthday.split_once('/')?;
    if birthday_month.parse::<u32>().ok()? != month
        || birthday_day.parse::<u32>().ok()? != day
        || birthday_count < 0
    {
        return None;
    }
    let encoded = character
        .birthday_bonus
        .split(';')
        .nth(birthday_count as usize)?
        .trim();
    (!encoded.is_empty()).then(|| reward::parse(encoded))
}
