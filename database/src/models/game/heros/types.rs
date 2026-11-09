use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

use super::{HeroModel, UserHeroModel};

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
    pub initial_voice_ids: Vec<i32>,
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

pub(super) fn passive_skill_levels(hero: &Hero) -> Vec<i32> {
    let level = config::configs::get()
        .character_rank_passive_level(hero.hero_id, hero.rank)
        .unwrap_or_default();
    (1..=level).collect()
}

pub(super) fn initial_voice_ids(game_data: &config::GameDB, hero_id: i32) -> Vec<i32> {
    game_data
        .character_voices_of_type(hero_id, config::CharacterVoiceType::Summon)
        .map(|row| row.audio)
        .collect()
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
