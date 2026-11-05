pub mod state;

use sonettobuf;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct UserExploreInfo {
    pub user_id: i64,
    pub last_map_id: i32,
    pub is_show_bag: bool,
}

#[derive(Debug, Clone, FromRow)]
pub struct ExploreChapter {
    pub chapter_id: i32,
    pub is_finish: bool,
}

#[derive(Debug, Clone)]
pub struct BonusSceneInfo {
    pub bonus_scene_id: i32,
    pub options: Vec<i32>,
}

impl From<BonusSceneInfo> for sonettobuf::BonusSceneNo {
    fn from(b: BonusSceneInfo) -> Self {
        sonettobuf::BonusSceneNo {
            bonus_scene_id: Some(b.bonus_scene_id),
            options: b.options,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExploreChapterSimple {
    pub chapter_id: i32,
    pub archive_ids: Vec<i32>,
    pub bonus_scenes: Vec<BonusSceneInfo>,
    pub is_finish: bool,
}

impl From<ExploreChapterSimple> for sonettobuf::ExploreChapterSimpleNo {
    fn from(c: ExploreChapterSimple) -> Self {
        sonettobuf::ExploreChapterSimpleNo {
            chapter_id: Some(c.chapter_id),
            archive_ids: c.archive_ids,
            bonus_scene: c.bonus_scenes.into_iter().map(Into::into).collect(),
            is_finish: Some(c.is_finish),
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct ExploreMap {
    pub map_id: i32,
    pub bonus_num: i32,
    pub gold_coin: i32,
    pub purple_coin: i32,
    pub bonus_num_total: i32,
    pub gold_coin_total: i32,
    pub purple_coin_total: i32,
}

#[derive(Debug, Clone)]
pub struct ExploreMapSimple {
    pub map: ExploreMap,
    pub bonus_ids: Vec<i32>,
}

impl From<ExploreMapSimple> for sonettobuf::ExploreMapSimpleNo {
    fn from(m: ExploreMapSimple) -> Self {
        sonettobuf::ExploreMapSimpleNo {
            map_id: Some(m.map.map_id),
            bonus_num: Some(m.map.bonus_num),
            gold_coin: Some(m.map.gold_coin),
            purple_coin: Some(m.map.purple_coin),
            bonus_num_total: Some(m.map.bonus_num_total),
            gold_coin_total: Some(m.map.gold_coin_total),
            purple_coin_total: Some(m.map.purple_coin_total),
            bonus_ids: m.bonus_ids,
        }
    }
}
