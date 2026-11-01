use crate::error::AppError;
use database::db::game::{explore, weekwalk, weekwalk_v2};
use sonettobuf::{
    GetExploreSimpleInfoReply, GetWeekwalkInfoReply, MarkPopDeepSettleReply,
    MarkPopShallowSettleReply, WeekwalkInfo, WeekwalkVer2GetInfoReply,
};
use sqlx::SqlitePool;

#[derive(Clone, Copy, Debug)]
pub struct ExplorationManager {
    player_id: i64,
}

impl ExplorationManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }

    pub async fn simple_info(self, db: &SqlitePool) -> Result<GetExploreSimpleInfoReply, AppError> {
        explore_simple_info(db, self.player_id).await
    }

    pub async fn weekwalk_info(self, db: &SqlitePool) -> Result<GetWeekwalkInfoReply, AppError> {
        weekwalk_info(db, self.player_id).await
    }

    pub async fn mark_pop_shallow_settle(
        self,
        db: &SqlitePool,
    ) -> Result<MarkPopShallowSettleReply, AppError> {
        weekwalk::mark_pop_shallow_settle(db, self.player_id).await?;
        Ok(MarkPopShallowSettleReply {})
    }

    pub async fn mark_pop_deep_settle(
        self,
        db: &SqlitePool,
    ) -> Result<MarkPopDeepSettleReply, AppError> {
        weekwalk::mark_pop_deep_settle(db, self.player_id).await?;
        Ok(MarkPopDeepSettleReply {})
    }

    pub async fn weekwalk_v2_info(
        self,
        db: &SqlitePool,
    ) -> Result<WeekwalkVer2GetInfoReply, AppError> {
        weekwalk_v2_info(db, self.player_id).await
    }
}

async fn explore_simple_info(
    db: &SqlitePool,
    player_id: i64,
) -> Result<GetExploreSimpleInfoReply, AppError> {
    let (info, chapters, maps, unlock_map_ids) = explore::get_explore_info(db, player_id).await?;

    // Present the first map before its detailed state is created. Existing
    // exploration summaries are never replaced by these initial values.
    if info.last_map_id == 0 && chapters.is_empty() && maps.is_empty() && unlock_map_ids.is_empty() {
        let opened: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM user_open_infos WHERE user_id = ? AND open_id = 47 AND is_open = 1)",
        ).bind(player_id).fetch_one(db).await?;
        if opened {
            let layout = config::explore_layout::layout(101)?
                .ok_or_else(|| AppError::Custom("initial exploration map is missing".into()))?;
            let (bonus, gold, purple) = crate::explore_entry::map_totals(layout)?;
            return Ok(GetExploreSimpleInfoReply {
                last_map_id: Some(0),
                chapter_simple: vec![sonettobuf::ExploreChapterSimpleNo {
                    chapter_id: Some(layout.chapter_id), archive_ids: vec![], bonus_scene: vec![], is_finish: Some(false),
                }],
                map_simple: vec![sonettobuf::ExploreMapSimpleNo {
                    map_id: Some(layout.map_id), bonus_num: Some(0), gold_coin: Some(0), purple_coin: Some(0),
                    bonus_num_total: Some(bonus), gold_coin_total: Some(gold), purple_coin_total: Some(purple),
                    bonus_ids: vec![],
                }],
                unlock_map_ids: vec![layout.map_id],
                is_show_bag: Some(info.is_show_bag),
            });
        }
    }

    Ok(GetExploreSimpleInfoReply {
        last_map_id: Some(info.last_map_id),
        chapter_simple: chapters.into_iter().map(Into::into).collect(),
        map_simple: maps.into_iter().map(Into::into).collect(),
        unlock_map_ids,
        is_show_bag: Some(info.is_show_bag),
    })
}

async fn weekwalk_info(db: &SqlitePool, player_id: i64) -> Result<GetWeekwalkInfoReply, AppError> {
    weekwalk::ensure_initial_shallow_map(db, player_id).await?;
    let (info, map_info) = weekwalk::get_weekwalk_info(db, player_id).await?;

    Ok(GetWeekwalkInfoReply {
        info: Some(WeekwalkInfo {
            time: Some(info.time),
            end_time: Some(info.end_time),
            map_info,
            max_layer: Some(info.max_layer),
            issue_id: Some(info.issue_id),
            is_pop_deep_rule: Some(info.is_pop_deep_rule),
            is_open_deep: Some(info.is_open_deep),
            is_pop_shallow_settle: Some(info.is_pop_shallow_settle),
            is_pop_deep_settle: Some(info.is_pop_deep_settle),
            deep_progress: Some(info.deep_progress),
        }),
        time_this_week: Some(info.time_this_week),
    })
}

async fn weekwalk_v2_info(
    db: &SqlitePool,
    player_id: i64,
) -> Result<WeekwalkVer2GetInfoReply, AppError> {
    Ok(WeekwalkVer2GetInfoReply {
        info: Some(weekwalk_v2::get_weekwalk_v2_info(db, player_id).await?),
    })
}
