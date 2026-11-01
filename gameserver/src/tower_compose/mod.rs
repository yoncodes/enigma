use crate::error::AppError;
use crate::player::battle::ActiveBattle;
use database::db::game::tower_compose::{self, TowerComposeThemeState};
use serde::Deserialize;
use sonettobuf::{
    TowerComposeBoss, TowerComposeFightSettlePush, TowerComposeGetInfoReply, TowerComposeInfo,
    TowerComposePlaneMods, TowerComposeSetModsReply, TowerComposeTheme,
};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::HashMap;

mod boss;

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
    let online = tables
        .tower_compose_theme
        .iter()
        .filter(|theme| theme.is_online == 1)
        .map(|theme| theme.id)
        .collect::<Vec<_>>();
    let mut tx = db.begin().await?;
    let mut themes = Vec::with_capacity(online.len());
    for id in online {
        let state = states.get(&id);
        let mut theme = theme_info(id, state);
        let layer = boss::active_layer(&mut tx, player_id, id, state).await?;
        theme.boss = Some(boss::load_boss(&mut tx, player_id, id, layer, state).await?);
        theme.curr_record = boss::saved_record(&mut tx, player_id, id).await?;
        theme.saved_record = Some(theme.curr_record.is_some());
        themes.push(theme);
    }
    tx.commit().await?;

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

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ComposeBattle {
    theme_id: i32,
    layer_id: i32,
    plane_id: i32,
    #[serde(default)]
    support_id: i32,
    #[serde(default)]
    research_id: i32,
    #[serde(default)]
    support_assist_uid: i64,
}

fn compose_context(episode_id: i32, params: Option<&str>) -> Option<ComposeBattle> {
    let battle: ComposeBattle = serde_json::from_str(params?).ok()?;
    config::configs::get()
        .tower_compose_episode
        .iter()
        .any(|episode| {
            episode.theme_id == battle.theme_id
                && episode.layer_id == battle.layer_id
                && boss::plane_matches(episode.plane, battle.plane_id)
                && episode.episode_id == episode_id
        })
        .then_some(battle)
}

fn compose_battle(active: &ActiveBattle) -> Option<ComposeBattle> {
    compose_context(active.episode_id, active.params.as_deref())
}

pub async fn ensure_unlocked(
    db: &SqlitePool,
    player_id: i64,
    episode_id: i32,
    params: Option<&str>,
) -> Result<(), AppError> {
    let tables = config::configs::get();
    if !tables.tower_compose_episode.iter().any(|row| row.episode_id == episode_id) {
        return Ok(());
    }
    let context = compose_context(episode_id, params).ok_or(AppError::InvalidRequest)?;
    let theme = tables.tower_compose_theme.get(context.theme_id).ok_or(AppError::InvalidRequest)?;
    if theme.is_online != 1 {
        return Err(AppError::InvalidRequest);
    }
    let row = tables.tower_compose_episode.iter().find(|row| {
        row.theme_id == context.theme_id && row.layer_id == context.layer_id
            && row.episode_id == episode_id && boss::plane_matches(row.plane, context.plane_id)
    }).ok_or(AppError::InvalidRequest)?;
    let state = tower_compose::get_theme_state(db, player_id, context.theme_id).await?;
    let cleared = state.as_ref().map_or(0, |state| state.pass_max_layer_id);
    let high_score = state.as_ref().map_or(0, |state| state.high_score);
    let required_score = if row.unlock.trim().is_empty() {
        0
    } else {
        row.unlock.trim().parse::<i32>().map_err(|_| AppError::InvalidRequest)?
    };
    if context.layer_id > cleared.saturating_add(1) || high_score < required_score {
        return Err(AppError::InvalidRequest);
    }
    Ok(())
}

pub fn prepare_fight_data(fight: &mut sonettobuf::Fight, episode_id: i32, params: Option<&str>) -> Result<(), AppError> {
    let tables = config::configs::get();
    if !tables.tower_compose_episode.iter().any(|row| row.episode_id == episode_id) {
        return Ok(());
    }
    let context = compose_context(episode_id, params).ok_or(AppError::InvalidRequest)?;
    let row = tables.tower_compose_episode.iter()
        .find(|row| row.theme_id == context.theme_id && row.layer_id == context.layer_id
            && row.episode_id == episode_id && boss::plane_matches(row.plane, context.plane_id))
        .ok_or(AppError::InvalidRequest)?;
    let kind = sonettobuf::custom_data::CustomDataType::TowerCompose as i32;
    fight.custom_data.retain(|data| data.r#type != Some(kind));
    fight.custom_data.push(sonettobuf::CustomData {
        r#type: Some(kind),
        data: Some(serde_json::json!({"themeId":context.theme_id,"layerId":context.layer_id,
            "planeId":context.plane_id,"maxPlaneId":row.plane}).to_string()),
    });
    Ok(())
}

pub async fn battle_data(
    db: &SqlitePool, player_id: i64, episode_id: i32, params: Option<&str>,
) -> Result<Option<battle::dungeon::compose::ComposeFightData>, AppError> {
    let tables = config::configs::get();
    if !tables.tower_compose_episode.iter().any(|row| row.episode_id == episode_id) {
        return Ok(None);
    }
    let context = compose_context(episode_id, params).ok_or(AppError::InvalidRequest)?;
    let row = tables.tower_compose_episode.iter().find(|row| {
        row.theme_id == context.theme_id && row.layer_id == context.layer_id
            && row.episode_id == episode_id && boss::plane_matches(row.plane, context.plane_id)
    }).ok_or(AppError::InvalidRequest)?;
    let mut tx = db.begin().await?;
    let state = tower_compose::get_theme_state_in_transaction(&mut tx, player_id, context.theme_id).await?;
    if context.research_id != 0 {
        let research = tables.tower_compose_research.get(context.research_id)
            .filter(|research| research.theme_id == context.theme_id)
            .ok_or(AppError::InvalidRequest)?;
        if research.req > state.as_ref().map_or(0, |state| state.research_progress) {
            return Err(AppError::InvalidRequest);
        }
    }
    let mods = if context.plane_id > 0 {
        let boss = boss::load_boss(&mut tx, player_id, context.theme_id, context.layer_id, state.as_ref()).await?;
        boss.planes.into_iter().find(|plane| plane.plane_id == Some(context.plane_id))
            .ok_or(AppError::InvalidRequest)?.mods
    } else { Vec::new() };
    tx.commit().await?;
    Ok(Some(battle::dungeon::compose::ComposeFightData {
        theme_id: context.theme_id, layer_id: context.layer_id, plane_id: context.plane_id,
        max_plane_id: row.plane, research_id: context.research_id, mods,
    }))
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
        return boss::settle(tx, player_id, active, context).await.map(Some);
    }
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
    database::db::game::tasks::sync_tower_compose_tasks_in_transaction(tx, player_id, None).await?;
    let state = tower_compose::get_theme_state_in_transaction(tx, player_id, context.theme_id).await?;
    let mut theme = theme_info(context.theme_id, state.as_ref());
    let layer = boss::active_layer(tx, player_id, context.theme_id, state.as_ref()).await?;
    theme.boss = Some(boss::load_boss(tx, player_id, context.theme_id, layer, state.as_ref()).await?);
    theme.curr_record = boss::saved_record(tx, player_id, context.theme_id).await?;
    theme.saved_record = Some(theme.curr_record.is_some());

    Ok(Some(TowerComposeFightSettlePush {
        theme: Some(theme),
        boss_settle: Some(sonettobuf::TowerComposeBossSettle { planes: vec![], new_flag: Some(false), record: None }),
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
    boss::validate_mods(db, player_id, theme_id, &plane_mods).await?;
    tower_compose::save_plane_mods(db, player_id, theme_id, &plane_mods).await?;
    let mut tx = db.begin().await?;
    let state = tower_compose::get_theme_state_in_transaction(&mut tx, player_id, theme_id).await?;
    let layer = boss::active_layer(&mut tx, player_id, theme_id, state.as_ref()).await?;
    let boss = boss::load_boss(&mut tx, player_id, theme_id, layer, state.as_ref()).await?;
    let submitted = plane_mods.iter().map(|plane| (plane.plane_id,
        plane.mods.iter().flat_map(|group| group.mods.iter())
            .filter_map(|item| item.mod_id.filter(|id| *id > 0)).collect::<Vec<_>>()
    )).collect::<Vec<_>>();
    let returned = boss.planes.iter().map(|plane| (plane.plane_id,
        plane.mods.iter().flat_map(|group| group.mods.iter())
            .filter_map(|item| item.mod_id.filter(|id| *id > 0)).collect::<Vec<_>>()
    )).collect::<Vec<_>>();
    tracing::info!(theme_id, layer, submitted = ?submitted, returned = ?returned,
        "Tower Compose component save");
    tx.commit().await?;

    Ok(TowerComposeSetModsReply {
        theme_id: Some(theme_id),
        level: boss.level,
        planes: boss.planes,
    })
}

fn max_selected_mod_level(plane_mods: &[TowerComposePlaneMods]) -> i32 {
    plane_mods
        .iter()
        .map(|plane| {
            plane.mods.iter().flat_map(|mods| mods.mods.iter())
                .filter_map(|selected| config::configs::get().tower_compose_mod.get(selected.mod_id?))
                .map(|config| config.level)
                .sum::<i32>()
        })
        .max()
        .unwrap_or_default()
}

pub async fn ensure_team_available(db: &SqlitePool, player_id: i64, active: &ActiveBattle) -> Result<(), AppError> {
    boss::ensure_team_available(db, player_id, active).await
}

pub async fn update_record(db: &SqlitePool, player_id: i64, theme_id: i32, update: bool) -> Result<sonettobuf::TowerComposeUpdateRecordReply, AppError> {
    Ok(sonettobuf::TowerComposeUpdateRecordReply { theme_id: Some(theme_id), record: boss::update_record(db, player_id, theme_id, update).await? })
}

pub async fn load_record(db: &SqlitePool, player_id: i64, theme_id: i32) -> Result<sonettobuf::TowerComposeLoadRecordReply, AppError> {
    Ok(sonettobuf::TowerComposeLoadRecordReply { theme_id: Some(theme_id), record: boss::get_saved_record(db, player_id, theme_id).await? })
}

pub async fn re_challenge(db: &SqlitePool, player_id: i64, theme_id: i32, plane_id: i32) -> Result<sonettobuf::TowerComposeReChallengeReply, AppError> {
    Ok(sonettobuf::TowerComposeReChallengeReply { theme_id: Some(theme_id), boss: Some(boss::re_challenge(db, player_id, theme_id, plane_id).await?) })
}

pub async fn cancel_re_challenge(db: &SqlitePool, player_id: i64, theme_id: i32) -> Result<sonettobuf::TowerComposeCancelReChallengeReply, AppError> {
    Ok(sonettobuf::TowerComposeCancelReChallengeReply { theme_id: Some(theme_id), boss: Some(boss::cancel_re_challenge(db, player_id, theme_id).await?) })
}

#[cfg(test)]
mod tests;
