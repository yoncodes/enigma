use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use sonettobuf;
use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};

#[async_trait::async_trait]
pub trait HeroModel<T>: Send + Sync {
    async fn get(&self, hero_id: i32) -> Result<T>;
    async fn get_uid(&self, hero_uid: i64) -> Result<T>;
    async fn get_all(&self) -> Result<Vec<T>>;
    async fn has_hero(&self, hero_id: i32) -> Result<bool>;
    async fn hero_duplicate(&self, hero_id: i32) -> Result<i32>;
    async fn create_hero(&self, tx: &mut Transaction<'_, Sqlite>, hero_id: i32) -> Result<i64>;
    async fn hero_count(&self, rarity: usize, now: i64) -> Result<()>;
    async fn special_equipped_gear(&self, hero_id: i32, extra_str: String) -> Result<()>;
    async fn equipped_gear(&self, hero_id: i32, equip_uid: i64) -> Result<bool>;
    async fn touch_count(&self) -> Result<Option<i32>>;
    async fn use_touch(
        &self,
        hero_id: i32,
        faith_amount: i32,
        max_faith: i32,
    ) -> Result<Option<i32>>;
    async fn skin(&self, hero_id: i32, skin_id: i32) -> Result<bool>;
    async fn skins(&self) -> Result<Vec<i32>>;
    async fn birthdays(&self) -> Result<Vec<(i32, i32)>>;
    async fn destiny_stone(&self, hero_id: i32, stone_id: i32) -> Result<()>;
    async fn update_destiny_progress(&self, hero_id: i32, rank: i32, level: i32) -> Result<()>;
    async fn unlock_destiny_stone(&self, hero_id: i32, stone_id: i32) -> Result<bool>;
    async fn level_up(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        hero_id: i32,
        current_level: i32,
        new_level: i32,
    ) -> Result<bool>;
    async fn read_hero_red_dot(&self, hero_id: i32, red_dot: i32) -> Result<()>;
    async fn upgrade_ex_skill(&self, hero_id: i32, levels: i32) -> Result<()>;
    async fn set_favor(&self, hero_id: i32, is_favor: bool) -> Result<()>;
    async fn unmark_new(&self, hero_id: i32) -> Result<()>;
    async fn set_show_hero(&self, hero_uids: Vec<i64>) -> Result<()>;
    async fn talent_style_read(&self, hero_id: i32) -> Result<()>;
    async fn update_talent(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        hero_id: i32,
        current_talent: i32,
        talent_id: i32,
    ) -> Result<bool>;
    async fn remove_talent_cube(
        &self,
        hero_id: i32,
        template_id: i32,
        pos_x: i32,
        pos_y: i32,
    ) -> Result<()>;
    async fn place_talent_cube(
        &self,
        hero_id: i32,
        template_id: i32,
        cube_id: i32,
        direction: i32,
        pos_x: i32,
        pos_y: i32,
    ) -> Result<()>;
    async fn sync_active_talent_cubes(
        &self,
        hero_id: i32,
        template_id: i32,
        get_cube: Option<(i32, i32)>,
        put_cube: Option<(i32, i32, i32, i32)>,
    ) -> Result<()>;
    async fn replace_talent_cubes(
        &self,
        hero_id: i32,
        template_id: i32,
        cubes: Vec<(i32, i32, i32, i32)>,
    ) -> Result<sonettobuf::TalentTemplateInfo>;
    async fn get_template_info(
        &self,
        hero_id: i32,
        template_id: i32,
    ) -> Result<sonettobuf::TalentTemplateInfo>;
    async fn rename_talent_template(
        &self,
        hero_id: i32,
        template_id: i32,
        name: &str,
    ) -> Result<sonettobuf::TalentTemplateInfo>;
    async fn load_talent_scheme(
        &self,
        hero_id: i32,
        talent_id: i32,
        talent_mould: i32,
        template_id: i32,
    ) -> Result<sonettobuf::TalentTemplateInfo>;
    async fn has_talent_style(&self, hero_id: i32, style: i32) -> Result<bool>;
    async fn unlock_talent_style(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        hero_id: i32,
        style: i32,
    ) -> Result<bool>;
    async fn apply_talent_style(&self, hero_id: i32, template_id: i32, style: i32) -> Result<()>;
    async fn switch_talent_template(
        &self,
        hero_id: i32,
        template_id: i32,
    ) -> Result<sonettobuf::TalentTemplateInfo>;
}

pub struct UserHeroModel {
    user_id: i64,
    pool: SqlitePool,
}

pub struct InsightUpgrade {
    pub item_uid: i64,
    pub item_id: i32,
    pub hero_id: i32,
    pub current_rank: i32,
    pub current_level: i32,
    pub target_rank: i32,
    pub target_level: i32,
}

pub struct HeroGrant {
    pub is_new: bool,
    pub duplicate_count: i32,
}

pub async fn get_hero_by_uid(pool: &SqlitePool, hero_uid: i64) -> Result<HeroData> {
    let user_id: i64 = sqlx::query_scalar("SELECT user_id FROM heroes WHERE uid = ?")
        .bind(hero_uid)
        .fetch_one(pool)
        .await?;
    UserHeroModel::new(user_id, pool.clone())
        .get_uid(hero_uid)
        .await
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Hero {
    pub uid: i64,
    pub user_id: i64,
    pub hero_id: i32,
    pub create_time: i64,
    pub level: i32,
    pub exp: i32,
    pub rank: i32,
    pub breakthrough: i32,
    pub skin: i32,
    pub faith: i32,
    pub active_skill_level: i32,
    pub ex_skill_level: i32,
    pub is_new: bool,
    pub talent: i32,
    pub default_equip_uid: i64,
    pub duplicate_count: i32,
    pub use_talent_template_id: i32,
    pub talent_style_unlock: i32,
    pub talent_style_red: i32,
    pub is_favor: bool,
    pub destiny_rank: i32,
    pub destiny_level: i32,
    pub destiny_stone: i32,
    pub red_dot: i32,
    pub extra_str: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct HeroSkin {
    pub hero_uid: i64,
    pub skin: i32,
    pub expire_sec: i32,
}

impl From<HeroSkin> for sonettobuf::SkinInfo {
    fn from(s: HeroSkin) -> Self {
        sonettobuf::SkinInfo {
            skin: Some(s.skin),
            expire_sec: Some(s.expire_sec),
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct HeroTalentCube {
    pub hero_uid: i64,
    pub cube_id: i32,
    pub direction: i32,
    pub pos_x: i32,
    pub pos_y: i32,
}

impl From<HeroTalentCube> for sonettobuf::TalentCubeInfo {
    fn from(c: HeroTalentCube) -> Self {
        sonettobuf::TalentCubeInfo {
            cube_id: Some(c.cube_id),
            direction: Some(c.direction),
            pos_x: Some(c.pos_x),
            pos_y: Some(c.pos_y),
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct HeroTalentTemplate {
    pub id: i64,
    pub hero_uid: i64,
    pub template_id: i32,
    pub name: String,
    pub style: i32,
}

#[derive(Debug, Clone)]
pub struct HeroData {
    pub record: Hero,
    pub passive_skill_levels: Vec<i32>,
    pub voices: Vec<i32>,
    pub voices_heard: Vec<i32>,
    pub skin_list: Vec<HeroSkin>,
    pub item_unlocks: Vec<i32>,
    pub talent_cubes: Vec<HeroTalentCube>,
    pub talent_templates: Vec<(HeroTalentTemplate, Vec<HeroTalentCube>)>,
    pub destiny_stone_unlocks: Vec<i32>,
}

impl HeroData {
    pub fn into_proto(
        self,
        base_attr: sonettobuf::HeroAttribute,
        ex_attr: sonettobuf::HeroExAttribute,
        sp_attr: sonettobuf::HeroSpAttribute,
    ) -> sonettobuf::HeroInfo {
        let h = self;
        sonettobuf::HeroInfo {
            uid: h.record.uid,
            user_id: h.record.user_id,
            hero_id: h.record.hero_id,
            create_time: Some(h.record.create_time),
            level: Some(h.record.level),
            exp: Some(h.record.exp),
            rank: Some(h.record.rank),
            breakthrough: Some(h.record.breakthrough),
            skin: Some(h.record.skin),
            faith: Some(h.record.faith),
            active_skill_level: Some(h.record.active_skill_level),
            passive_skill_level: h.passive_skill_levels,
            ex_skill_level: Some(h.record.ex_skill_level),
            voice: h.voices,
            voice_heard: h.voices_heard,
            skin_info_list: h.skin_list.into_iter().map(Into::into).collect(),
            base_attr: Some(base_attr),
            ex_attr: Some(ex_attr),
            sp_attr: Some(sp_attr),
            equip_attr_list: Vec::new(),
            is_new: Some(h.record.is_new),
            item_unlock: h.item_unlocks,
            talent: Some(h.record.talent),
            talent_cube_infos: h.talent_cubes.into_iter().map(Into::into).collect(),
            default_equip_uid: Some(h.record.default_equip_uid),
            duplicate_count: Some(h.record.duplicate_count),
            talent_templates: h
                .talent_templates
                .into_iter()
                .map(|(template, cubes)| sonettobuf::TalentTemplateInfo {
                    id: Some(template.template_id),
                    talent_cube_infos: cubes.into_iter().map(Into::into).collect(),
                    name: Some(template.name),
                    style: Some(template.style),
                })
                .collect(),
            use_talent_template_id: Some(h.record.use_talent_template_id),
            talent_style_unlock: Some(h.record.talent_style_unlock),
            talent_style_red: Some(h.record.talent_style_red),
            is_favor: Some(h.record.is_favor),
            destiny_rank: Some(h.record.destiny_rank),
            destiny_level: Some(h.record.destiny_level),
            destiny_stone: Some(h.record.destiny_stone),
            destiny_stone_unlock: h.destiny_stone_unlocks,
            red_dot: Some(h.record.red_dot),
            extra_str: Some(h.record.extra_str),
        }
    }
}

impl UserHeroModel {
    pub fn new(user_id: i64, pool: SqlitePool) -> Self {
        Self { user_id, pool }
    }

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
            });
        }

        self.create_hero_in_transaction(tx, hero_id).await?;
        Ok(HeroGrant {
            is_new: true,
            duplicate_count: 0,
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

    pub async fn update_skin(&self, hero_uid: i32, skin_id: i32) -> Result<bool> {
        HeroModel::<HeroData>::skin(self, hero_uid, skin_id).await
    }

    pub async fn set_favor(&self, hero_id: i32, is_favor: bool) -> Result<()> {
        HeroModel::<HeroData>::set_favor(self, hero_id, is_favor).await
    }

    pub async fn unmark_new(&self, hero_id: i32) -> Result<()> {
        HeroModel::<HeroData>::unmark_new(self, hero_id).await
    }

    pub async fn read_red_dot(&self, hero_id: i32, red_dot: i32) -> Result<()> {
        HeroModel::<HeroData>::read_hero_red_dot(self, hero_id, red_dot).await
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

    async fn attach_owned_skins_in_transaction(
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

    pub async fn get_birthdays(&self) -> Result<Vec<(i32, i32)>> {
        HeroModel::<HeroData>::birthdays(self).await
    }

    pub async fn update_destiny_stone(&self, hero_id: i32, stone_id: i32) -> Result<()> {
        HeroModel::<HeroData>::destiny_stone(self, hero_id, stone_id).await
    }

    pub async fn set_rank_and_level(&self, hero_id: i32, rank: i32, level: i32) -> Result<()> {
        sqlx::query("UPDATE heroes SET rank = ?, level = ? WHERE user_id = ? AND hero_id = ?")
            .bind(rank)
            .bind(level)
            .bind(self.user_id)
            .bind(hero_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn rank_up_with_insight_skin_in_transaction(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        hero_id: i32,
        current_rank: i32,
    ) -> Result<bool> {
        let new_rank = current_rank + 1;
        let updated = sqlx::query(
            "UPDATE heroes SET rank = ?, level = level + 1
             WHERE user_id = ? AND hero_id = ? AND rank = ?",
        )
        .bind(new_rank)
        .bind(self.user_id)
        .bind(hero_id)
        .bind(current_rank)
        .execute(&mut **tx)
        .await?;
        if updated.rows_affected() != 1 {
            return Ok(false);
        }

        if new_rank >= 3
            && let Some(skin_id) = config::configs::get()
                .skin
                .iter()
                .find(|skin| {
                    skin.character_id == hero_id && skin.id % 100 == 2 && skin.gain_approach == 1
                })
                .map(|skin| skin.id)
        {
            self.unlock_skin_in_transaction(tx, skin_id).await?;
        }

        Ok(true)
    }

    pub async fn apply_insight_item(&self, upgrade: InsightUpgrade) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        let consumed = sqlx::query(
            "UPDATE insight_items
             SET quantity = quantity - 1
             WHERE user_id = ? AND uid = ? AND item_id = ? AND quantity >= 1",
        )
        .bind(self.user_id)
        .bind(upgrade.item_uid)
        .bind(upgrade.item_id)
        .execute(&mut *tx)
        .await?;
        if consumed.rows_affected() != 1 {
            return Ok(false);
        }

        let hero_uid: Option<i64> = sqlx::query_scalar(
            "UPDATE heroes
             SET rank = ?, level = ?
             WHERE user_id = ? AND hero_id = ? AND rank = ? AND level = ?
             RETURNING uid",
        )
        .bind(upgrade.target_rank)
        .bind(upgrade.target_level)
        .bind(self.user_id)
        .bind(upgrade.hero_id)
        .bind(upgrade.current_rank)
        .bind(upgrade.current_level)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(_hero_uid) = hero_uid else {
            return Ok(false);
        };

        if upgrade.target_rank >= 3
            && let Some(skin_id) = config::configs::get()
                .skin
                .iter()
                .find(|skin| {
                    skin.character_id == upgrade.hero_id
                        && skin.id % 100 == 2
                        && skin.gain_approach == 1
                })
                .map(|skin| skin.id)
        {
            self.unlock_skin_in_transaction(&mut tx, skin_id).await?;
        }

        tx.commit().await?;
        Ok(true)
    }

    pub async fn upgrade_ex_skill_in_transaction(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        hero_id: i32,
        current_level: i32,
        levels: i32,
    ) -> Result<bool> {
        Ok(sqlx::query(
            "UPDATE heroes
             SET ex_skill_level = ex_skill_level + ?
             WHERE user_id = ? AND hero_id = ? AND ex_skill_level = ?
               AND ex_skill_level + ? <= 5",
        )
        .bind(levels)
        .bind(self.user_id)
        .bind(hero_id)
        .bind(current_level)
        .bind(levels)
        .execute(&mut **tx)
        .await?
        .rows_affected()
            == 1)
    }

    pub async fn update_destiny_progress_in_transaction(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        hero_id: i32,
        current_rank: i32,
        current_level: i32,
        rank: i32,
        level: i32,
    ) -> Result<bool> {
        Ok(sqlx::query(
            "UPDATE heroes
             SET destiny_rank = ?, destiny_level = ?
             WHERE user_id = ? AND hero_id = ?
               AND destiny_rank = ? AND destiny_level = ?",
        )
        .bind(rank)
        .bind(level)
        .bind(self.user_id)
        .bind(hero_id)
        .bind(current_rank)
        .bind(current_level)
        .execute(&mut **tx)
        .await?
        .rows_affected()
            == 1)
    }

    pub async fn unlock_destiny_stone_in_transaction(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        hero_id: i32,
        stone_id: i32,
    ) -> Result<bool> {
        let hero_uid: Option<i64> =
            sqlx::query_scalar("SELECT uid FROM heroes WHERE user_id = ? AND hero_id = ?")
                .bind(self.user_id)
                .bind(hero_id)
                .fetch_optional(&mut **tx)
                .await?;
        let Some(hero_uid) = hero_uid else {
            return Ok(false);
        };
        Ok(sqlx::query(
            "INSERT OR IGNORE INTO hero_destiny_stone_unlocks (hero_uid, stone_id)
             VALUES (?, ?)",
        )
        .bind(hero_uid)
        .bind(stone_id)
        .execute(&mut **tx)
        .await?
        .rows_affected()
            == 1)
    }
}

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

        let passive_skill_levels: Vec<i32> = sqlx::query_scalar(
            "SELECT level FROM hero_passive_skill_levels WHERE hero_uid = ? ORDER BY skill_index",
        )
        .bind(hero_uid)
        .fetch_all(&self.pool)
        .await?;

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

        let passive_skill_levels: Vec<i32> = sqlx::query_scalar(
            "SELECT level FROM hero_passive_skill_levels WHERE hero_uid = ? ORDER BY skill_index",
        )
        .bind(hero_uid)
        .fetch_all(&self.pool)
        .await?;

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

            let passive_skill_levels: Vec<i32> = sqlx::query_scalar(
                "SELECT level FROM hero_passive_skill_levels WHERE hero_uid = ?1 ORDER BY skill_index",
            )
            .bind(hero_uid)
            .fetch_all(&self.pool)
            .await?;

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

        for voice in game_data
            .character_voices(hero_id)
            .filter(|row| row.r#type == 9 || row.r#type == 11)
        {
            sqlx::query("INSERT INTO hero_voices (hero_uid, voice_id) VALUES (?, ?)")
                .bind(hero_uid)
                .bind(voice.audio)
                .execute(&mut **tx)
                .await?;
        }

        sqlx::query(
            "INSERT INTO hero_birthday_info (user_id, hero_id, birthday_count) VALUES (?, ?, ?)",
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
        if config::configs::get()
            .skin
            .get(skin_id)
            .is_none_or(|skin| skin.character_id != hero_id)
        {
            return Ok(false);
        }

        let result = sqlx::query(
            "UPDATE heroes SET skin = ?
             WHERE hero_id = ? AND user_id = ?
               AND EXISTS(
                   SELECT 1 FROM hero_all_skins
                   WHERE hero_all_skins.user_id = heroes.user_id
                     AND hero_all_skins.skin_id = ?
               )",
        )
        .bind(skin_id)
        .bind(hero_id)
        .bind(self.user_id)
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

    async fn read_hero_red_dot(&self, hero_id: i32, red_dot: i32) -> Result<()> {
        let hero_data = self.get(hero_id).await?;
        sqlx::query("UPDATE heroes SET red_dot = ? WHERE uid = ? AND user_id = ?")
            .bind(red_dot)
            .bind(hero_data.record.uid)
            .bind(self.user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
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
        let hero_data = self.get(hero_id).await?;

        sqlx::query("UPDATE heroes SET talent_style_red = 0 WHERE uid = ? AND user_id = ?")
            .bind(hero_data.record.uid)
            .bind(self.user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn update_talent(
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

    async fn remove_talent_cube(
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

    async fn place_talent_cube(
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

    async fn sync_active_talent_cubes(
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

    async fn replace_talent_cubes(
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

        self.get_template_info(hero_id, template_id).await
    }

    async fn get_template_info(
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

    async fn rename_talent_template(
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
        self.get_template_info(hero_id, template_id).await
    }

    async fn load_talent_scheme(
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

    async fn has_talent_style(&self, hero_id: i32, style: i32) -> Result<bool> {
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

    async fn unlock_talent_style(
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

    async fn apply_talent_style(&self, hero_id: i32, template_id: i32, style: i32) -> Result<()> {
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

    async fn switch_talent_template(
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
