use anyhow::{Result, anyhow};
use sonettobuf;
use sqlx::{Sqlite, SqlitePool, Transaction};

mod affinity;
mod lifecycle;
mod profile;
mod progression;
mod skins;
mod storage;
mod talent;
mod types;

pub use types::*;

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
    async fn read_hero_red_dot(&self, hero_id: i32, red_dot_type: i32) -> Result<i32>;
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

impl UserHeroModel {
    pub fn new(user_id: i64, pool: SqlitePool) -> Self {
        Self { user_id, pool }
    }
}
