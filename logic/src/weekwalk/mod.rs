use crate::error::AppError;
use common::time::ServerTime;
use config::{GameDB, episode::Episode};
use database::db::game::{
    hero_groups,
    weekwalk::{self, WeekwalkBattleSelection, WeekwalkMapSeed},
};
use serde::{Deserialize, Serialize};
use sonettobuf::{
    BeforeStartWeekwalkBattleReply, ChangeWeekwalkHeroGroupSelectReply, GetWeekwalkInfoReply,
    MarkPopDeepSettleReply, MarkPopShallowSettleReply, WeekwalkInfo, WeekwalkInfoUpdatePush,
};
use sqlx::SqlitePool;

const CYCLE_SECONDS: i32 = 30 * 24 * 60 * 60;

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
        self.pending = Some(
            weekwalk::select_battle(db, self.player_id, map_id, element_id)
                .await?
                .ok_or(AppError::InvalidRequest)?
                .into(),
        );
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
        let map = cycle.maps[index];
        let expected_battle = if context.element_id == map.front_element_id {
            map.front_battle_id
        } else if context.element_id == map.rear_element_id {
            map.rear_battle_id
        } else {
            return Err(AppError::InvalidRequest);
        };
        if context.battle_id != expected_battle
            || !weekwalk::complete_battle(
                db,
                self.player_id,
                context.into(),
                star,
                fight_id,
                map.rear_element_id,
                cycle.maps.get(index + 1).copied(),
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

    async fn ensure_cycle(&self, db: &SqlitePool) -> Result<(), AppError> {
        let cycle = current_cycle(config::configs::get())?;
        let now = ServerTime::now_sec_i32();
        weekwalk::ensure_cycle(
            db,
            self.player_id,
            cycle.issue_id,
            now,
            now.saturating_add(CYCLE_SECONDS),
            cycle.maps[0],
        )
        .await?;
        Ok(())
    }
}

pub fn is_weekwalk_episode(episode: &Episode) -> bool {
    episode.r#type == 9
}

struct WeekwalkCycle {
    issue_id: i32,
    maps: Vec<WeekwalkMapSeed>,
}

fn current_cycle(tables: &GameDB) -> Result<WeekwalkCycle, AppError> {
    let issue_id = tables
        .weekwalk
        .iter()
        .filter(|map| map.issue_id > 0 && map.fight_id_front > 0 && map.fight_id_rear > 0)
        .map(|map| map.issue_id)
        .max()
        .ok_or_else(|| AppError::Custom("no usable Weekwalk issue configured".into()))?;
    let mut maps = tables
        .weekwalk
        .iter()
        .filter(|map| map.issue_id == issue_id)
        .collect::<Vec<_>>();
    maps.sort_unstable_by_key(|map| map.layer);
    let first_layer = maps.first().ok_or(AppError::InvalidRequest)?.layer;
    let scene_family = maps.first().ok_or(AppError::InvalidRequest)?.scene_id / 10;
    let mut front = battle_elements(tables, scene_family, 1);
    let mut rear = battle_elements(tables, scene_family, 2);
    front.sort_unstable();
    rear.sort_unstable();

    let maps = maps
        .into_iter()
        .map(|map| {
            let index =
                usize::try_from(map.layer - first_layer).map_err(|_| AppError::InvalidRequest)?;
            Ok(WeekwalkMapSeed {
                map_id: map.id,
                scene_id: map.scene_id,
                layer: map.layer,
                front_battle_id: map.fight_id_front,
                front_element_id: *front.get(index).ok_or(AppError::InvalidRequest)?,
                rear_battle_id: map.fight_id_rear,
                rear_element_id: *rear.get(index).ok_or(AppError::InvalidRequest)?,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    Ok(WeekwalkCycle { issue_id, maps })
}

fn battle_elements(tables: &GameDB, scene_family: i32, round_id: i32) -> Vec<i32> {
    tables
        .weekwalk_element
        .iter()
        .filter(|element| {
            element.id / 100_000 == scene_family
                && element.round_id == round_id
                && element.skip_finish == 0
                && element.r#type.split('#').any(|kind| kind == "1")
        })
        .map(|element| element.id)
        .collect()
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
