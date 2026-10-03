use crate::error::AppError;
use crate::player::battle::ActiveBattle;
use database::db::game::tower_compose::{self, TowerComposeThemeState};
use serde::Deserialize;
use sonettobuf::{
    TowerComposeBoss, TowerComposeFightSettlePush, TowerComposeGetInfoReply, TowerComposeInfo,
    TowerComposePlane, TowerComposePlaneMods, TowerComposeSetModsReply, TowerComposeTheme,
};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::HashMap;

pub async fn get_info(
    db: &SqlitePool,
    player_id: i64,
) -> Result<TowerComposeGetInfoReply, AppError> {
    let states: HashMap<i32, TowerComposeThemeState> =
        tower_compose::get_theme_states(db, player_id)
            .await?
            .into_iter()
            .map(|state| (state.theme_id, state))
            .collect();

    let tables = config::configs::get();
    let themes = tables
        .tower_compose_theme
        .iter()
        .filter(|theme| theme.is_online == 1)
        .map(|theme| theme_info(theme.id, states.get(&theme.id)))
        .collect();

    Ok(TowerComposeGetInfoReply {
        info: Some(TowerComposeInfo { themes }),
    })
}

fn theme_info(theme_id: i32, state: Option<&TowerComposeThemeState>) -> TowerComposeTheme {
    let pass_max_layer_id = state
        .map(|state| state.pass_max_layer_id)
        .unwrap_or_default();
    TowerComposeTheme {
        theme_id: Some(theme_id),
        research_progress: Some(
            state
                .map(|state| state.research_progress)
                .unwrap_or_default(),
        ),
        unlock_mod_ids: unlock_mods(theme_id, pass_max_layer_id),
        boss: Some(TowerComposeBoss {
            planes: vec![],
            high_score: Some(state.map(|state| state.high_score).unwrap_or_default()),
            curr_score: Some(state.map(|state| state.curr_score).unwrap_or_default()),
            level: Some(state.map(|state| state.boss_level).unwrap_or_default()),
            lock: Some(state.map(|state| state.boss_lock).unwrap_or_default()),
        }),
        curr_record: None,
        saved_record: Some(state.map(|state| state.saved_record).unwrap_or_default()),
        pass_max_layer_id: Some(pass_max_layer_id),
        params: state
            .filter(|state| !state.params.is_empty())
            .map(|state| state.params.clone()),
    }
}

fn unlock_mods(theme_id: i32, pass_max_layer_id: i32) -> Vec<i32> {
    let tables = config::configs::get();
    let mut ids = tables
        .tower_compose_mod
        .iter()
        .filter(|config| config.theme_id == theme_id && config.is_unlock == 1)
        .map(|config| config.id)
        .collect::<Vec<_>>();
    ids.extend(
        tables
            .tower_compose_episode
            .iter()
            .filter(|episode| episode.theme_id == theme_id && episode.layer_id <= pass_max_layer_id)
            .flat_map(|episode| episode.unlock_mod_ids.split('|'))
            .filter_map(|id| id.parse::<i32>().ok()),
    );
    ids
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ComposeBattle {
    theme_id: i32,
    layer_id: i32,
    plane_id: i32,
}

fn compose_battle(active: &ActiveBattle) -> Option<ComposeBattle> {
    let battle: ComposeBattle = serde_json::from_str(active.params.as_deref()?).ok()?;
    config::configs::get()
        .tower_compose_episode
        .iter()
        .any(|episode| {
            episode.theme_id == battle.theme_id
                && episode.layer_id == battle.layer_id
                && episode.plane == battle.plane_id
                && episode.episode_id == active.episode_id
        })
        .then_some(battle)
}

pub fn matches_battle(active: &ActiveBattle) -> bool {
    compose_battle(active).is_some()
}

pub async fn settle_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    player_id: i64,
    active: &ActiveBattle,
) -> Result<Option<TowerComposeFightSettlePush>, AppError> {
    let Some(context) = compose_battle(active) else {
        return Ok(None);
    };
    if context.plane_id > 0 {
        tracing::warn!(
            theme_id = context.theme_id,
            layer_id = context.layer_id,
            plane_id = context.plane_id,
            "Tower Compose boss-plane settlement is not implemented"
        );
        return Ok(None);
    }
    // Live sends the pre-settlement theme snapshot; the committed layer is visible to GetInfo.
    let state =
        tower_compose::get_theme_state_in_transaction(tx, player_id, context.theme_id).await?;
    let result = if active.runtime.outcome() == battle::BattleOutcome::Victory {
        tower_compose::complete_layer_in_transaction(
            tx,
            player_id,
            context.theme_id,
            context.layer_id,
        )
        .await?;
        1
    } else {
        2
    };

    Ok(Some(TowerComposeFightSettlePush {
        theme: Some(theme_info(context.theme_id, state.as_ref())),
        boss_settle: None,
        result: Some(result),
        params: active.params.clone(),
    }))
}

pub async fn set_mods(
    db: &SqlitePool,
    player_id: i64,
    theme_id: i32,
    plane_mods: Vec<TowerComposePlaneMods>,
) -> Result<TowerComposeSetModsReply, AppError> {
    tower_compose::save_plane_mods(db, player_id, theme_id, &plane_mods).await?;

    Ok(TowerComposeSetModsReply {
        theme_id: Some(theme_id),
        level: Some(max_selected_mod_level(&plane_mods)),
        planes: plane_mods
            .iter()
            .map(|plane| TowerComposePlane {
                plane_id: plane.plane_id,
                mods: plane.mods.clone(),
                team: None,
                curr_score: Some(0),
                result: Some(0),
                lock: Some(false),
            })
            .collect(),
    })
}

fn max_selected_mod_level(plane_mods: &[TowerComposePlaneMods]) -> i32 {
    plane_mods
        .iter()
        .flat_map(|plane| plane.mods.iter())
        .flat_map(|mods| mods.mods.iter())
        .filter_map(|selected| {
            config::configs::get()
                .tower_compose_mod
                .get(selected.mod_id?)
        })
        .map(|config| config.level)
        .max()
        .unwrap_or_default()
}

#[cfg(test)]
mod test;
