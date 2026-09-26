// Auto-generated from JSON data
// Do not edit manually

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skin {
    #[serde(rename = "alternateSpine")]
    pub alternate_spine: String,
    #[serde(rename = "alternateSpineJump")]
    pub alternate_spine_jump: String,
    #[serde(rename = "cameraSize")]
    pub camera_size: i32,
    #[serde(rename = "canHide")]
    pub can_hide: i32,
    #[serde(rename = "canWade")]
    pub can_wade: i32,
    #[serde(rename = "characterDataTitleViewOffset")]
    pub character_data_title_view_offset: String,
    #[serde(rename = "characterDataVoiceViewOffset")]
    pub character_data_voice_view_offset: String,
    #[serde(rename = "characterGetViewOffset")]
    pub character_get_view_offset: String,
    #[serde(rename = "characterGetViewStaticOffset")]
    pub character_get_view_static_offset: String,
    #[serde(rename = "characterId")]
    pub character_id: i32,
    #[serde(rename = "characterRankUpViewOffset")]
    pub character_rank_up_view_offset: String,
    #[serde(rename = "characterSkin")]
    pub character_skin: String,
    #[serde(rename = "characterSkinNameEng")]
    pub character_skin_name_eng: String,
    #[serde(rename = "characterTitleViewStaticOffset")]
    pub character_title_view_static_offset: String,
    #[serde(rename = "characterViewImgOffset")]
    pub character_view_img_offset: String,
    #[serde(rename = "characterViewOffset")]
    pub character_view_offset: String,
    pub color: String,
    pub colorbg: i32,
    pub compensate: String,
    pub costafterbuy: String,
    #[serde(rename = "defaultStencilValue")]
    pub default_stencil_value: String,
    pub des: String,
    pub drawing: String,
    pub effect: String,
    #[serde(rename = "effectHangPoint")]
    pub effect_hang_point: String,
    #[serde(rename = "fightSuccViewOffset")]
    pub fight_succ_view_offset: String,
    pub fight_special: i32,
    #[serde(rename = "flipX")]
    pub flip_x: i32,
    #[serde(rename = "focusOffset")]
    pub focus_offset: Vec<serde_json::Value>,
    #[serde(rename = "folderName")]
    pub folder_name: String,
    #[serde(rename = "frameStencilValue")]
    pub frame_stencil_value: String,
    #[serde(rename = "fullScreenCameraSize")]
    pub full_screen_camera_size: i32,
    #[serde(rename = "fullScreenLive2dOffset")]
    pub full_screen_live2d_offset: String,
    #[serde(rename = "gainApproach")]
    pub gain_approach: i32,
    #[serde(rename = "guideLeftPortraitOffset")]
    pub guide_left_portrait_offset: String,
    #[serde(rename = "guideRightPortraitOffset")]
    pub guide_right_portrait_offset: String,
    #[serde(rename = "haloOffset")]
    pub halo_offset: String,
    #[serde(rename = "handbookRedDot")]
    pub handbook_red_dot: i32,
    #[serde(rename = "headIcon")]
    pub head_icon: String,
    pub id: i32,
    #[serde(rename = "isFly")]
    pub is_fly: i32,
    #[serde(rename = "isSkinVideo")]
    pub is_skin_video: bool,
    #[serde(rename = "itemIcon")]
    pub item_icon: String,
    pub jump: i32,
    #[serde(rename = "largeIcon")]
    pub large_icon: String,
    pub live2d: String,
    pub live2dbg: String,
    #[serde(rename = "lucidescapeViewImgOffset")]
    pub lucidescape_view_img_offset: String,
    #[serde(rename = "mainBody")]
    pub main_body: i32,
    #[serde(rename = "mainThumbnailViewOffset")]
    pub main_thumbnail_view_offset: String,
    #[serde(rename = "mainViewFrameOffset")]
    pub main_view_frame_offset: String,
    #[serde(rename = "mainViewOffset")]
    pub main_view_offset: String,
    #[serde(rename = "matId")]
    pub mat_id: i32,
    pub name: String,
    #[serde(rename = "nameEng")]
    pub name_eng: String,
    #[serde(rename = "noDeadEffect")]
    pub no_dead_effect: i32,
    #[serde(rename = "playercardViewImgOffset")]
    pub playercard_view_img_offset: String,
    #[serde(rename = "playercardViewLive2dOffset")]
    pub playercard_view_live2d_offset: String,
    pub rare: i32,
    #[serde(rename = "repeatBuyTime")]
    pub repeat_buy_time: String,
    #[serde(rename = "retangleIcon")]
    pub retangle_icon: String,
    #[serde(rename = "showDrawingSwitch")]
    pub show_drawing_switch: i32,
    #[serde(rename = "showSwitchBtn")]
    pub show_switch_btn: i32,
    #[serde(rename = "showTemplate")]
    pub show_template: i32,
    pub skills: String,
    #[serde(rename = "skin2dParams")]
    pub skin2d_params: String,
    #[serde(rename = "skinDescription")]
    pub skin_description: String,
    #[serde(rename = "skinGainViewImgOffset")]
    pub skin_gain_view_img_offset: String,
    #[serde(rename = "skinGetBackIcon")]
    pub skin_get_back_icon: String,
    #[serde(rename = "skinGetColorbg")]
    pub skin_get_colorbg: String,
    #[serde(rename = "skinGetDetailViewIconOffset")]
    pub skin_get_detail_view_icon_offset: String,
    #[serde(rename = "skinGetDetailViewSpineOffset")]
    pub skin_get_detail_view_spine_offset: String,
    #[serde(rename = "skinGetIcon")]
    pub skin_get_icon: String,
    #[serde(rename = "skinLevel")]
    pub skin_level: i32,
    #[serde(rename = "skinReplaceIcon")]
    pub skin_replace_icon: String,
    #[serde(rename = "skinSignature")]
    pub skin_signature: String,
    #[serde(rename = "skinSpineOffset")]
    pub skin_spine_offset: String,
    #[serde(rename = "skinStoreId")]
    pub skin_store_id: i32,
    #[serde(rename = "skinStoreViewImgOffset")]
    pub skin_store_view_img_offset: String,
    #[serde(rename = "skinStory")]
    pub skin_story: String,
    #[serde(rename = "skinSwitchLive2dOffset")]
    pub skin_switch_live2d_offset: String,
    #[serde(rename = "skinViewImgOffset")]
    pub skin_view_img_offset: String,
    pub spine: String,
    #[serde(rename = "storeTag")]
    pub store_tag: String,
    #[serde(rename = "subTitle")]
    pub sub_title: String,
    #[serde(rename = "summonHeroViewOffset")]
    pub summon_hero_view_offset: String,
    #[serde(rename = "summonPickUpImgOffset")]
    pub summon_pick_up_img_offset: String,
    #[serde(rename = "topuiOffset")]
    pub topui_offset: Option<serde_json::Value>,
    #[serde(rename = "triggerArea1")]
    pub trigger_area1: String,
    #[serde(rename = "triggerArea2")]
    pub trigger_area2: String,
    #[serde(rename = "triggerArea3")]
    pub trigger_area3: String,
    #[serde(rename = "triggerArea4")]
    pub trigger_area4: String,
    #[serde(rename = "triggerArea5")]
    pub trigger_area5: String,
    #[serde(rename = "unavailableStore")]
    pub unavailable_store: String,
    #[serde(rename = "unlockCondition")]
    pub unlock_condition: String,
    #[serde(rename = "verticalDrawing")]
    pub vertical_drawing: String,
    #[serde(rename = "weatherParam")]
    pub weather_param: i32,
}
use std::collections::HashMap;

pub struct SkinTable {
    records: Vec<Skin>,
    by_id: HashMap<i32, usize>,
}

impl SkinTable {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let records: Vec<Skin> = crate::load_rows(path)?;

        let mut by_id = HashMap::with_capacity(records.len());

        for (idx, record) in records.iter().enumerate() {
            by_id.insert(record.id, idx);
        }

        Ok(Self {
            records,
            by_id,
        })
    }

    #[inline]
    pub fn get(&self, id: i32) -> Option<&Skin> {
        self.by_id.get(&id).map(|&i| &self.records[i])
    }

    #[inline]
    pub fn all(&self) -> &[Skin] {
        &self.records
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Skin> {
        self.records.iter()
    }

    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}