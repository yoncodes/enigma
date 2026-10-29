use crate::error::AppError;
use common::time::ServerTime;
use config::episode::Episode;
use database::db::game::{
    hero_groups,
    weekwalk::{self, WeekwalkBattleSelection, WeekwalkElementState},
};
use serde::{Deserialize, Serialize};
use sonettobuf::{
    BeforeStartWeekwalkBattleReply, ChangeWeekwalkHeroGroupSelectReply, GetWeekwalkInfoReply,
    MarkPopDeepSettleReply, MarkPopShallowSettleReply, WeekwalkDialogHistoryReply,
    WeekwalkDialogReply, WeekwalkInfo, WeekwalkInfoUpdatePush,
};
use sqlx::SqlitePool;

const CYCLE_SECONDS: i32 = 30 * 24 * 60 * 60;

mod cycle;

use cycle::{current as current_cycle, dialog_option_valid, element_step};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeekwalkBattleContext {
    pub map_id: i32,
    pub element_id: i32,
    pub battle_id: i32,
}

impl From<WeekwalkBattleSelection> for WeekwalkBattleContext {
    fn from(value: WeekwalkBattleSelection) -> Self {
        Self {
            map_id: value.map_id,
            element_id: value.element_id,
            battle_id: value.battle_id,
        }
    }
}

impl From<WeekwalkBattleContext> for WeekwalkBattleSelection {
    fn from(value: WeekwalkBattleContext) -> Self {
        Self {
            map_id: value.map_id,
            element_id: value.element_id,
            battle_id: value.battle_id,
        }
    }
}

#[derive(Clone, Debug)]
pub struct WeekwalkManager {
    player_id: i64,
    pending: Option<WeekwalkBattleContext>,
}

impl WeekwalkManager {
    pub fn new(player_id: i64) -> Self {
        Self {
            player_id,
            pending: None,
        }
    }

    pub async fn info(&self, db: &SqlitePool) -> Result<GetWeekwalkInfoReply, AppError> {
        self.ensure_cycle(db).await?;
        let (info, map_info) = weekwalk::get_weekwalk_info(db, self.player_id).await?;
        let time_this_week = info.time_this_week;
        Ok(GetWeekwalkInfoReply {
            info: Some(project_info(info, map_info)),
            time_this_week: Some(time_this_week),
        })
    }

    pub async fn select_battle(
        &mut self,
        db: &SqlitePool,
        map_id: i32,
        element_id: i32,
    ) -> Result<BeforeStartWeekwalkBattleReply, AppError> {
        self.ensure_cycle(db).await?;
        let selection = weekwalk::select_battle(db, self.player_id, map_id, element_id)
            .await?
            .ok_or(AppError::InvalidRequest)?;
        let cycle = current_cycle(config::configs::get())?;
        let map = cycle
            .maps
            .iter()
            .find(|map| map.map_id == map_id)
            .ok_or(AppError::InvalidRequest)?;
        let encounter = map
            .encounters
            .iter()
            .find(|encounter| encounter.element_id == element_id)
            .ok_or(AppError::InvalidRequest)?;
        let index = self.element_index(db, map_id, element_id).await?;
        let (kind, configured_battle_id, _) =
            element_step(config::configs::get(), element_id, index)?;
        if kind != 1
            || encounter.battle_id != selection.battle_id
            || (!map.is_deep && configured_battle_id != selection.battle_id)
        {
            return Err(AppError::InvalidRequest);
        }
        self.pending = Some(selection.into());
        Ok(BeforeStartWeekwalkBattleReply {
            element_id: Some(element_id),
            layer_id: Some(map_id),
        })
    }

    pub fn selected_battle(&self) -> Result<WeekwalkBattleContext, AppError> {
        self.pending.ok_or(AppError::InvalidRequest)
    }

    pub fn clear_selection(&mut self) {
        self.pending = None;
    }

    pub async fn change_group_selection(
        &self,
        db: &SqlitePool,
        map_id: i32,
        battle_id: i32,
        selection: i32,
    ) -> Result<ChangeWeekwalkHeroGroupSelectReply, AppError> {
        self.ensure_cycle(db).await?;
        if selection < 0
            || (selection > 0
                && hero_groups::get_hero_group(db, self.player_id, selection)
                    .await?
                    .is_none())
            || !weekwalk::set_hero_group_selection(db, self.player_id, map_id, battle_id, selection)
                .await?
        {
            return Err(AppError::InvalidRequest);
        }
        Ok(ChangeWeekwalkHeroGroupSelectReply {
            layer_id: Some(map_id),
            battle_id: Some(battle_id),
            select: Some(selection),
        })
    }

    pub async fn mark_pop_shallow_settle(
        &self,
        db: &SqlitePool,
    ) -> Result<MarkPopShallowSettleReply, AppError> {
        weekwalk::mark_pop_shallow_settle(db, self.player_id).await?;
        Ok(MarkPopShallowSettleReply {})
    }

    pub async fn mark_pop_deep_settle(
        &self,
        db: &SqlitePool,
    ) -> Result<MarkPopDeepSettleReply, AppError> {
        weekwalk::mark_pop_deep_settle(db, self.player_id).await?;
        Ok(MarkPopDeepSettleReply {})
    }

    pub async fn complete_battle(
        &self,
        db: &SqlitePool,
        context: WeekwalkBattleContext,
        star: i32,
        fight_id: i64,
    ) -> Result<WeekwalkInfoUpdatePush, AppError> {
        let cycle = current_cycle(config::configs::get())?;
        let index = cycle
            .maps
            .iter()
            .position(|map| map.map_id == context.map_id)
            .ok_or(AppError::InvalidRequest)?;
        let map = &cycle.maps[index];
        let encounter = map
            .encounters
            .iter()
            .find(|encounter| encounter.element_id == context.element_id)
            .ok_or(AppError::InvalidRequest)?;
        let current_index = self
            .element_index(db, context.map_id, context.element_id)
            .await?;
        let (kind, configured_battle_id, step_count) =
            element_step(config::configs::get(), context.element_id, current_index)?;
        if kind != 1 || (!map.is_deep && configured_battle_id != encounter.battle_id) {
            return Err(AppError::InvalidRequest);
        }
        let next_index = current_index + 1;
        if context.battle_id != encounter.battle_id
            || !weekwalk::complete_battle(
                db,
                self.player_id,
                context.into(),
                star,
                fight_id,
                map.encounters
                    .iter()
                    .skip_while(|candidate| candidate.element_id != context.element_id)
                    .nth(1)
                    .filter(|candidate| !candidate.visible)
                    .map(|candidate| candidate.element_id),
                WeekwalkElementState {
                    current_index,
                    next_index,
                    is_finish: next_index >= step_count,
                    next_map: cycle.maps.get(index + 1).cloned(),
                },
            )
            .await?
        {
            return Err(AppError::InvalidRequest);
        }
        let (info, map_info) = weekwalk::get_weekwalk_info(db, self.player_id).await?;
        Ok(WeekwalkInfoUpdatePush {
            info: Some(project_info(info, map_info)),
        })
    }

    pub async fn dialog(
        &self,
        db: &SqlitePool,
        map_id: i32,
        element_id: i32,
        option: i32,
    ) -> Result<(WeekwalkDialogReply, WeekwalkInfoUpdatePush), AppError> {
        self.ensure_cycle(db).await?;
        let current_index = self.element_index(db, map_id, element_id).await?;
        let (kind, dialog_id, _) = element_step(config::configs::get(), element_id, current_index)?;
        if kind != 4 || !dialog_option_valid(config::configs::get(), dialog_id, option) {
            return Err(AppError::InvalidRequest);
        }
        self.advance_element(db, map_id, element_id, 4).await?;
        Ok((
            WeekwalkDialogReply {
                element_id: Some(element_id),
                option: Some(option),
                layer_id: Some(map_id),
            },
            self.update(db).await?,
        ))
    }

    pub async fn store_dialog_history(
        &self,
        db: &SqlitePool,
        map_id: i32,
        element_id: i32,
        history: Vec<String>,
    ) -> Result<WeekwalkDialogHistoryReply, AppError> {
        self.ensure_cycle(db).await?;
        if !weekwalk::replace_element_history(db, self.player_id, map_id, element_id, &history)
            .await?
        {
            return Err(AppError::InvalidRequest);
        }
        Ok(WeekwalkDialogHistoryReply {
            element_id: Some(element_id),
            historylist: history,
            layer_id: Some(map_id),
        })
    }

    async fn advance_element(
        &self,
        db: &SqlitePool,
        map_id: i32,
        element_id: i32,
        expected_kind: i32,
    ) -> Result<(), AppError> {
        self.ensure_cycle(db).await?;
        let cycle = current_cycle(config::configs::get())?;
        let map_index = cycle
            .maps
            .iter()
            .position(|map| map.map_id == map_id)
            .ok_or(AppError::InvalidRequest)?;
        let current_index = self.element_index(db, map_id, element_id).await?;
        let (kind, _, step_count) =
            element_step(config::configs::get(), element_id, current_index)?;
        if kind != expected_kind
            || !weekwalk::advance_element(
                db,
                self.player_id,
                map_id,
                element_id,
                WeekwalkElementState {
                    current_index,
                    next_index: current_index + 1,
                    is_finish: current_index + 1 >= step_count,
                    next_map: cycle.maps.get(map_index + 1).cloned(),
                },
            )
            .await?
        {
            return Err(AppError::InvalidRequest);
        }
        Ok(())
    }

    async fn element_index(
        &self,
        db: &SqlitePool,
        map_id: i32,
        element_id: i32,
    ) -> Result<i32, AppError> {
        let (_, maps) = weekwalk::get_weekwalk_info(db, self.player_id).await?;
        maps.into_iter()
            .find(|map| map.id == Some(map_id))
            .and_then(|map| {
                map.element_infos
                    .into_iter()
                    .find(|element| element.element_id == Some(element_id))
            })
            .and_then(|element| element.index)
            .ok_or(AppError::InvalidRequest)
    }

    async fn update(&self, db: &SqlitePool) -> Result<WeekwalkInfoUpdatePush, AppError> {
        let (info, maps) = weekwalk::get_weekwalk_info(db, self.player_id).await?;
        Ok(WeekwalkInfoUpdatePush {
            info: Some(project_info(info, maps)),
        })
    }

    async fn ensure_cycle(&self, db: &SqlitePool) -> Result<(), AppError> {
        let cycle = current_cycle(config::configs::get())?;
        let now = ServerTime::now_sec_i32();
        weekwalk::ensure_cycle(
            db,
            self.player_id,
            cycle.issue_id,
            now,
            now.saturating_add(CYCLE_SECONDS),
            &cycle.maps[0],
        )
        .await?;
        Ok(())
    }
}

pub fn is_weekwalk_episode(episode: &Episode) -> bool {
    episode.r#type == 9
}

fn project_info(
    info: database::models::game::weekwalk::UserWeekwalkInfo,
    map_info: Vec<sonettobuf::MapInfo>,
) -> WeekwalkInfo {
    WeekwalkInfo {
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
    }
}

#[cfg(test)]
mod tests;
