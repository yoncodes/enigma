use crate::error::AppError;
use database::db::game::rouge;
use sonettobuf::{
    GetRouge2InfoReply, GetRouge2OutsideInfoReply, GetRougeOutsideInfoReply, Rouge2AlchemyInfo,
    Rouge2AlchemyMaterialInfo, Rouge2AttrInfo, Rouge2BagInfo, Rouge2BossBattleInfo,
    Rouge2CareerLevelInfo, Rouge2GetUnlockCollectionsReply, Rouge2Info, Rouge2OutsideInfo,
    Rouge2RewardInfo, Rouge2TotalRecordInfo, RougeOutsideInfo,
};
use sqlx::SqlitePool;

#[derive(Clone, Copy, Debug)]
pub struct RougeManager {
    player_id: i64,
}

impl RougeManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }

    pub async fn outside_info(
        self,
        db: &SqlitePool,
    ) -> Result<GetRouge2OutsideInfoReply, AppError> {
        rouge2_outside_info(db, self.player_id).await
    }

    pub async fn info(self, db: &SqlitePool) -> Result<GetRouge2InfoReply, AppError> {
        rouge2_info(db, self.player_id).await
    }

    pub async fn unlock_collections(
        self,
        db: &SqlitePool,
    ) -> Result<Rouge2GetUnlockCollectionsReply, AppError> {
        rouge2_unlock_collections(db, self.player_id).await
    }
}

pub fn rouge_outside_info(season: i32) -> GetRougeOutsideInfoReply {
    GetRougeOutsideInfoReply {
        rouge_info: Some(RougeOutsideInfo {
            season: Some(season),
            genius_point: Some(0),
            genius_ids: Vec::new(),
            point: Some(0),
            have_get_point: Some(0),
            bonus: None,
            review: Vec::new(),
            game_record_info: None,
            is_genius_new_stage: Some(false),
            limiter_info: None,
            cur_extra_point: Some(0),
        }),
    }
}

async fn rouge2_outside_info(
    db: &SqlitePool,
    user_id: i64,
) -> Result<GetRouge2OutsideInfoReply, AppError> {
    let rows = rouge::get_or_create_rouge2_outside(db, user_id, config::configs::get()).await?;
    let state = rows.state;

    Ok(GetRouge2OutsideInfoReply {
        outside_info: Some(Rouge2OutsideInfo {
            genius_point: Some(state.genius_point),
            genius_ids: json_ids(&state.genius_ids),
            total_record_info: Some(Rouge2TotalRecordInfo {
                max_difficulty: Some(state.max_difficulty),
                pass_layer_id: json_ids(&state.pass_layer_ids),
                pass_event_id: json_ids(&state.pass_event_ids),
                pass_end_id: json_ids(&state.pass_end_ids),
                pass_entrust_id: json_ids(&state.pass_entrust_ids),
                last_game_time: Some(state.last_game_time),
                pass_collections: json_ids(&state.pass_collections),
                hotfix_str: Some(state.hotfix_str),
            }),
            career_level_info: rows
                .career_levels
                .into_iter()
                .map(|career| Rouge2CareerLevelInfo {
                    career_id: Some(career.career_id),
                    exp: Some(career.exp),
                })
                .collect(),
            reward_info: rows
                .rewards
                .into_iter()
                .map(|reward| Rouge2RewardInfo {
                    id: Some(reward.reward_id),
                    buy_count: Some(reward.buy_count),
                })
                .collect(),
            reward_point: Some(state.reward_point),
            alchemy_info: Some(Rouge2AlchemyInfo {
                cur_alchemy_info: None,
                alchemy_material_info: rows
                    .materials
                    .into_iter()
                    .map(|material| Rouge2AlchemyMaterialInfo {
                        id: Some(material.material_id),
                        num: Some(material.num),
                    })
                    .collect(),
            }),
            review: Vec::new(),
            boss_battle_info: Some(Rouge2BossBattleInfo {
                boss_info: Vec::new(),
                save_info: Vec::new(),
                use_save_index: Some(0),
            }),
        }),
    })
}

async fn rouge2_info(db: &SqlitePool, user_id: i64) -> Result<GetRouge2InfoReply, AppError> {
    let rows = rouge::get_or_create_rouge2_outside(db, user_id, config::configs::get()).await?;
    let state = rows.state;

    Ok(GetRouge2InfoReply {
        rouge2_info: Some(Rouge2Info {
            state: Some(state.state),
            difficulty: Some(state.difficulty),
            coin: Some(state.coin),
            map_info: None,
            bag_info: Some(Rouge2BagInfo { bags: Vec::new() }),
            end_id: Some(state.end_id),
            game_num: Some(state.game_num),
            leader_info: None,
            attr_info: Some(Rouge2AttrInfo { attr: Vec::new() }),
            alchemy_info: None,
        }),
    })
}

async fn rouge2_unlock_collections(
    db: &SqlitePool,
    user_id: i64,
) -> Result<Rouge2GetUnlockCollectionsReply, AppError> {
    let tables = config::configs::get();

    Ok(Rouge2GetUnlockCollectionsReply {
        unlock_relics_ids: rouge::get_or_create_unlock_ids(
            db,
            user_id,
            tables,
            rouge::Rouge2UnlockKind::Relic,
        )
        .await?,
        unlock_buff_ids: rouge::get_or_create_unlock_ids(
            db,
            user_id,
            tables,
            rouge::Rouge2UnlockKind::Buff,
        )
        .await?,
        unlock_active_skill_ids: rouge::get_or_create_unlock_ids(
            db,
            user_id,
            tables,
            rouge::Rouge2UnlockKind::ActiveSkill,
        )
        .await?,
    })
}

fn json_ids(value: &str) -> Vec<i32> {
    serde_json::from_str(value).unwrap_or_default()
}
