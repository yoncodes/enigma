use super::{ComposeBattle, compose_battle, max_selected_mod_level, theme_info};
use crate::{error::AppError, player::battle::ActiveBattle};
use database::db::game::tower_compose::{self, TowerComposeThemeState};
use sonettobuf::{
    TowerComposeAssistHero, TowerComposeBoss, TowerComposeBossRecord, TowerComposeBossSettle,
    TowerComposeFightSettlePush, TowerComposeHero, TowerComposeMod, TowerComposeMods, TowerComposePlane,
    TowerComposePlaneMods, TowerComposePlaneSettle, TowerComposeTeam,
};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

pub(super) fn plane_matches(configured: i32, selected: i32) -> bool {
    match configured {
        0 => selected == 0,
        1 => selected == 1,
        2 => selected == 1 || selected == 2,
        _ => false,
    }
}

pub(super) fn current_layer(theme_id: i32, pass_layer: i32) -> i32 {
    config::configs::get().tower_compose_episode.iter()
        .filter(|row| row.theme_id == theme_id && row.plane > 0 && row.layer_id <= pass_layer.saturating_add(1))
        .map(|row| row.layer_id).max().unwrap_or_default()
}

pub(super) async fn active_layer(
    tx: &mut Transaction<'_, Sqlite>, player_id: i64, theme_id: i32, state: Option<&TowerComposeThemeState>,
) -> Result<i32, AppError> {
    if state.is_some_and(|value| value.boss_lock) {
        let layer = sqlx::query_scalar::<_, i32>("SELECT resume_layer_id FROM user_tower_compose_boss_records WHERE user_id = ? AND theme_id = ?")
            .bind(player_id).bind(theme_id).fetch_optional(&mut **tx).await?.ok_or(AppError::InvalidRequest)?;
        if layer <= 0 { return Err(AppError::InvalidRequest); }
        return Ok(layer);
    }
    Ok(current_layer(theme_id, state.map(|value| value.pass_max_layer_id).unwrap_or(0)))
}

fn empty_team() -> TowerComposeTeam {
    TowerComposeTeam {
        heroes: vec![], support_id: Some(0), research_id: Some(0), player_skill: Some(0),
        support_assist_hero: Some(TowerComposeAssistHero { uid: 0, hero_id: 0, ..Default::default() }),
    }
}

async fn load_mods(
    tx: &mut Transaction<'_, Sqlite>, player_id: i64, theme_id: i32,
) -> Result<Vec<TowerComposePlaneMods>, AppError> {
    let rows = sqlx::query_as::<_, (i32, String)>(
        "SELECT plane_id, mods_json FROM user_tower_compose_plane_mods WHERE user_id = ? AND theme_id = ? ORDER BY plane_id"
    ).bind(player_id).bind(theme_id).fetch_all(&mut **tx).await?;
    let theme = config::configs::get().tower_compose_theme.get(theme_id)
        .ok_or(AppError::InvalidRequest)?;
    let environment = config::configs::get().tower_compose_mod.get(theme.init_env)
        .filter(|item| item.r#type == 3)
        .ok_or(AppError::InvalidRequest)?;
    let mut saved = HashMap::new();
    for (plane_id, json) in rows {
        saved.insert(plane_id, serde_json::from_str::<Vec<TowerComposeMods>>(&json)?);
    }
    Ok((1..=2).map(|plane_id| TowerComposePlaneMods {
        plane_id: Some(plane_id),
        mods: saved.remove(&plane_id).unwrap_or_else(|| vec![TowerComposeMods {
            r#type: Some(3),
            mods: vec![TowerComposeMod { slot: Some(1), mod_id: Some(environment.id) }],
        }]),
    }).collect())
}

async fn load_record(
    tx: &mut Transaction<'_, Sqlite>, player_id: i64, theme_id: i32, column: &str,
) -> Result<Option<TowerComposeBossRecord>, AppError> {
    let sql = match column {
        "saved" => "SELECT saved_record_json FROM user_tower_compose_boss_records WHERE user_id = ? AND theme_id = ?",
        "current" => "SELECT current_record_json FROM user_tower_compose_boss_records WHERE user_id = ? AND theme_id = ?",
        _ => return Err(AppError::InvalidRequest),
    };
    let raw = sqlx::query_scalar::<_, String>(sql).bind(player_id).bind(theme_id)
        .fetch_optional(&mut **tx).await?.unwrap_or_default();
    if raw.is_empty() { Ok(None) } else { Ok(Some(serde_json::from_str(&raw)?)) }
}

pub(super) async fn saved_record(
    tx: &mut Transaction<'_, Sqlite>, player_id: i64, theme_id: i32,
) -> Result<Option<TowerComposeBossRecord>, AppError> {
    load_record(tx, player_id, theme_id, "saved").await
}

pub(super) async fn update_record(
    db: &SqlitePool, player_id: i64, theme_id: i32, update: bool,
) -> Result<Option<TowerComposeBossRecord>, AppError> {
    let mut tx = db.begin().await?;
    if update {
        let current = load_record(&mut tx, player_id, theme_id, "current").await?
            .ok_or(AppError::InvalidRequest)?;
        sqlx::query("UPDATE user_tower_compose_boss_records SET saved_record_json = current_record_json, saved_layer_id = layer_id, updated_at = ? WHERE user_id = ? AND theme_id = ?")
            .bind(common::time::ServerTime::now_ms()).bind(player_id).bind(theme_id).execute(&mut *tx).await?;
        sqlx::query("UPDATE user_tower_compose_theme_state SET saved_record = 1 WHERE user_id = ? AND theme_id = ?")
            .bind(player_id).bind(theme_id).execute(&mut *tx).await?;
        tx.commit().await?;
        return Ok(Some(current));
    }
    let record = saved_record(&mut tx, player_id, theme_id).await?;
    tx.commit().await?;
    Ok(record)
}

pub(super) async fn get_saved_record(
    db: &SqlitePool, player_id: i64, theme_id: i32,
) -> Result<Option<TowerComposeBossRecord>, AppError> {
    let mut tx = db.begin().await?;
    let record = saved_record(&mut tx, player_id, theme_id).await?;
    tx.commit().await?;
    Ok(record)
}

#[derive(Serialize, Deserialize)]
struct ResumeState {
    target_layer: i32,
    target_boss: TowerComposeBoss,
    mods: Vec<TowerComposePlaneMods>,
}

async fn write_plane(
    tx: &mut Transaction<'_, Sqlite>, player_id: i64, theme_id: i32, layer_id: i32,
    plane: &TowerComposePlane,
) -> Result<(), AppError> {
    let id = plane.plane_id.filter(|id| (1..=2).contains(id)).ok_or(AppError::InvalidRequest)?;
    sqlx::query("INSERT INTO user_tower_compose_boss_planes (user_id, theme_id, layer_id, plane_id, plane_json, updated_at) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(user_id, theme_id, layer_id, plane_id) DO UPDATE SET plane_json = excluded.plane_json, updated_at = excluded.updated_at")
        .bind(player_id).bind(theme_id).bind(layer_id).bind(id).bind(serde_json::to_string(plane)?)
        .bind(common::time::ServerTime::now_ms()).execute(&mut **tx).await?;
    Ok(())
}

async fn replace_mods(
    tx: &mut Transaction<'_, Sqlite>, player_id: i64, theme_id: i32, mods: &[TowerComposePlaneMods],
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM user_tower_compose_plane_mods WHERE user_id = ? AND theme_id = ?")
        .bind(player_id).bind(theme_id).execute(&mut **tx).await?;
    for plane in mods {
        sqlx::query("INSERT INTO user_tower_compose_plane_mods (user_id, theme_id, plane_id, mods_json, updated_at) VALUES (?, ?, ?, ?, ?)")
            .bind(player_id).bind(theme_id).bind(plane.plane_id.unwrap_or_default())
            .bind(serde_json::to_string(&plane.mods)?).bind(common::time::ServerTime::now_ms())
            .execute(&mut **tx).await?;
    }
    Ok(())
}

pub(super) async fn re_challenge(
    db: &SqlitePool, player_id: i64, theme_id: i32, selected: i32,
) -> Result<TowerComposeBoss, AppError> {
    if !(1..=2).contains(&selected) { return Err(AppError::InvalidRequest); }
    let mut tx = db.begin().await?;
    let row = sqlx::query_as::<_, (i32, String)>(
        "SELECT saved_layer_id, saved_record_json FROM user_tower_compose_boss_records WHERE user_id = ? AND theme_id = ?"
    ).bind(player_id).bind(theme_id).fetch_optional(&mut *tx).await?.ok_or(AppError::InvalidRequest)?;
    let record: TowerComposeBossRecord = serde_json::from_str(&row.1)?;
    let saved = record.boss.ok_or(AppError::InvalidRequest)?;
    if row.0 <= 0 || !saved.planes.iter().any(|plane| plane.plane_id == Some(selected) && plane.result.unwrap_or(0) != 0) {
        return Err(AppError::InvalidRequest);
    }
    let state = tower_compose::get_theme_state_in_transaction(&mut tx, player_id, theme_id).await?;
    if state.as_ref().is_some_and(|state| state.boss_lock) { return Err(AppError::InvalidRequest); }
    let target_boss = load_boss(&mut tx, player_id, theme_id, row.0, state.as_ref()).await?;
    let resume = ResumeState { target_layer: row.0, target_boss, mods: load_mods(&mut tx, player_id, theme_id).await? };
    sqlx::query("UPDATE user_tower_compose_boss_records SET resume_layer_id = ?, resume_boss_json = ?, updated_at = ? WHERE user_id = ? AND theme_id = ?")
        .bind(row.0).bind(serde_json::to_string(&resume)?).bind(common::time::ServerTime::now_ms())
        .bind(player_id).bind(theme_id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM user_tower_compose_boss_planes WHERE user_id = ? AND theme_id = ? AND layer_id = ?")
        .bind(player_id).bind(theme_id).bind(row.0).execute(&mut *tx).await?;
    let mut mods = Vec::new();
    for mut plane in saved.planes {
        let plane_id = plane.plane_id.ok_or(AppError::InvalidRequest)?;
        mods.push(TowerComposePlaneMods { plane_id: Some(plane_id), mods: plane.mods.clone() });
        plane.lock = Some(plane_id != selected);
        write_plane(&mut tx, player_id, theme_id, row.0, &plane).await?;
    }
    replace_mods(&mut tx, player_id, theme_id, &mods).await?;
    sqlx::query("UPDATE user_tower_compose_theme_state SET boss_lock = 1, updated_at = ? WHERE user_id = ? AND theme_id = ?")
        .bind(common::time::ServerTime::now_ms()).bind(player_id).bind(theme_id).execute(&mut *tx).await?;
    let state = tower_compose::get_theme_state_in_transaction(&mut tx, player_id, theme_id).await?;
    let boss = load_boss(&mut tx, player_id, theme_id, row.0, state.as_ref()).await?;
    tx.commit().await?;
    Ok(boss)
}

pub(super) async fn cancel_re_challenge(
    db: &SqlitePool, player_id: i64, theme_id: i32,
) -> Result<TowerComposeBoss, AppError> {
    let mut tx = db.begin().await?;
    let raw = sqlx::query_scalar::<_, String>("SELECT resume_boss_json FROM user_tower_compose_boss_records WHERE user_id = ? AND theme_id = ?")
        .bind(player_id).bind(theme_id).fetch_optional(&mut *tx).await?.unwrap_or_default();
    if raw.is_empty() { return Err(AppError::InvalidRequest); }
    let resume: ResumeState = serde_json::from_str(&raw)?;
    sqlx::query("DELETE FROM user_tower_compose_boss_planes WHERE user_id = ? AND theme_id = ? AND layer_id = ?")
        .bind(player_id).bind(theme_id).bind(resume.target_layer).execute(&mut *tx).await?;
    for plane in &resume.target_boss.planes {
        if plane.result.unwrap_or_default() != 0 { write_plane(&mut tx, player_id, theme_id, resume.target_layer, plane).await?; }
    }
    replace_mods(&mut tx, player_id, theme_id, &resume.mods).await?;
    sqlx::query("UPDATE user_tower_compose_boss_records SET resume_layer_id = 0, resume_boss_json = '', updated_at = ? WHERE user_id = ? AND theme_id = ?")
        .bind(common::time::ServerTime::now_ms()).bind(player_id).bind(theme_id).execute(&mut *tx).await?;
    sqlx::query("UPDATE user_tower_compose_theme_state SET boss_lock = 0, updated_at = ? WHERE user_id = ? AND theme_id = ?")
        .bind(common::time::ServerTime::now_ms()).bind(player_id).bind(theme_id).execute(&mut *tx).await?;
    let state = tower_compose::get_theme_state_in_transaction(&mut tx, player_id, theme_id).await?;
    let layer = current_layer(theme_id, state.as_ref().map(|value| value.pass_max_layer_id).unwrap_or(0));
    let boss = load_boss(&mut tx, player_id, theme_id, layer, state.as_ref()).await?;
    tx.commit().await?;
    Ok(boss)
}

pub(super) async fn load_boss(
    tx: &mut Transaction<'_, Sqlite>, player_id: i64, theme_id: i32, layer_id: i32,
    state: Option<&TowerComposeThemeState>,
) -> Result<TowerComposeBoss, AppError> {
    let mods = load_mods(tx, player_id, theme_id).await?;
    let rows = sqlx::query_as::<_, (i32, String)>(
        "SELECT plane_id, plane_json FROM user_tower_compose_boss_planes WHERE user_id = ? AND theme_id = ? AND layer_id = ?"
    ).bind(player_id).bind(theme_id).bind(layer_id).fetch_all(&mut **tx).await?;
    let mut saved = HashMap::new();
    for (plane_id, raw) in rows {
        let plane: TowerComposePlane = serde_json::from_str(&raw)?;
        if plane.plane_id != Some(plane_id) { return Err(AppError::InvalidRequest); }
        saved.insert(plane_id, plane);
    }
    let configured_planes = config::configs::get().tower_compose_episode.iter()
        .find(|row| row.theme_id == theme_id && row.layer_id == layer_id)
        .map(|row| row.plane)
        .filter(|count| *count > 0)
        .unwrap_or(1);
    let planes = (1..=2).map(|plane_id| {
        let mut plane = saved.remove(&plane_id).unwrap_or_else(|| TowerComposePlane {
            plane_id: Some(plane_id), mods: vec![], team: Some(empty_team()),
            curr_score: Some(0), result: Some(0), lock: Some(false),
        });
        if !plane.lock.unwrap_or(false) {
            plane.mods = mods.iter().find(|m| m.plane_id == Some(plane_id))
                .map(|m| m.mods.clone()).unwrap_or_default();
        }
        // The client only sends selections for the configured number of
        // planes. Keep its second plane object for results, but don't expose
        // an editable second plane on the one-plane boss.
        if plane_id > configured_planes { plane.lock = Some(true); }
        if plane.team.is_none() { plane.team = Some(empty_team()); }
        plane
    }).collect::<Vec<_>>();
    Ok(TowerComposeBoss {
        curr_score: Some(planes.iter().map(|p| p.curr_score.unwrap_or(0)).sum()),
        high_score: Some(state.map(|s| s.high_score).unwrap_or(0)),
        level: Some(max_selected_mod_level(&mods)),
        lock: Some(state.map(|s| s.boss_lock).unwrap_or(false)),
        planes,
    })
}

fn team_from_active(active: &ActiveBattle, context: ComposeBattle) -> Result<TowerComposeTeam, AppError> {
    let group = active.fight_group.as_ref().ok_or(AppError::InvalidRequest)?;
    let (fight, _) = active.runtime.reconnect_state();
    let attacker = fight.attacker.ok_or(AppError::InvalidRequest)?;
    let mut heroes = Vec::new();
    for entity in attacker.entitys.iter().chain(&attacker.sub_entitys) {
        let uid = entity.uid.unwrap_or_default();
        let trial = entity.trial_id.unwrap_or_default();
        if !group.hero_list.contains(&uid) && !group.sub_hero_list.contains(&uid) && trial <= 0 {
            continue;
        }
        let position = entity.position.unwrap_or_default();
        if position <= 0 { continue; }
        heroes.push(TowerComposeHero {
            pos: Some(position), hero_id: Some(if trial > 0 { 0 } else { entity.model_id.unwrap_or_default() }),
            trial_id: Some(trial), mind_id: Some(entity.equip_uid.unwrap_or_default()),
        });
    }
    heroes.sort_by_key(|hero| hero.pos.unwrap_or_default());
    let support_assist_hero = if context.support_assist_uid > 0 {
        let entity = attacker.assist_boss.ok_or(AppError::InvalidRequest)?;
        let support = config::configs::get().tower_compose_support.get(context.support_id)
            .ok_or(AppError::InvalidRequest)?;
        TowerComposeAssistHero {
            uid: context.support_assist_uid, hero_id: support.hero_id,
            level: entity.level, skin: entity.skin, ex_skill_level: entity.ex_skill_level,
            ..Default::default()
        }
    } else {
        TowerComposeAssistHero { uid: 0, hero_id: 0, ..Default::default() }
    };
    Ok(TowerComposeTeam {
        heroes, support_id: Some(context.support_id), research_id: Some(context.research_id),
        player_skill: group.cloth_id, support_assist_hero: Some(support_assist_hero),
    })
}

fn team_hero_ids(team: &TowerComposeTeam) -> Result<HashSet<i32>, AppError> {
    let tables = config::configs::get();
    let mut ids = HashSet::new();
    for hero in &team.heroes {
        let trial = hero.trial_id.unwrap_or_default();
        let id = if trial > 0 {
            tables.hero_trial.get(trial).ok_or(AppError::InvalidRequest)?.hero_id
        } else { hero.hero_id.unwrap_or_default() };
        if id > 0 { ids.insert(id); }
    }
    if let Some(support) = tables.tower_compose_support.get(team.support_id.unwrap_or_default()) {
        ids.insert(support.hero_id);
    }
    Ok(ids)
}

fn selected_mods(mods: &[TowerComposeMods]) -> Vec<(i32, i32, i32)> {
    let mut result = mods.iter().flat_map(|group| group.mods.iter().filter_map(move |item| {
        let id = item.mod_id.unwrap_or_default();
        (id > 0).then_some((group.r#type.unwrap_or_default(), item.slot.unwrap_or_default(), id))
    })).collect::<Vec<_>>();
    result.sort_unstable();
    result
}

pub(super) async fn validate_mods(
    db: &SqlitePool, player_id: i64, theme_id: i32, requested: &[TowerComposePlaneMods],
) -> Result<(), AppError> {
    let mut tx = db.begin().await?;
    let state = tower_compose::get_theme_state_in_transaction(&mut tx, player_id, theme_id).await?;
    if state.as_ref().is_some_and(|value| value.boss_lock) {
        let layer = sqlx::query_scalar::<_, i32>("SELECT resume_layer_id FROM user_tower_compose_boss_records WHERE user_id = ? AND theme_id = ?")
            .bind(player_id).bind(theme_id).fetch_one(&mut *tx).await?;
        let boss = load_boss(&mut tx, player_id, theme_id, layer, state.as_ref()).await?;
        for locked in boss.planes.iter().filter(|plane| plane.lock.unwrap_or(false)) {
            let Some(supplied) = requested.iter().find(|plane| plane.plane_id == locked.plane_id) else {
                continue;
            };
            if selected_mods(&locked.mods) != selected_mods(&supplied.mods) {
                return Err(AppError::InvalidRequest);
            }
        }
    }
    tx.commit().await?;
    Ok(())
}

pub(super) async fn ensure_team_available(
    db: &SqlitePool, player_id: i64, active: &ActiveBattle,
) -> Result<(), AppError> {
    let Some(context) = compose_battle(active) else { return Ok(()); };
    if context.plane_id <= 0 { return Ok(()); }
    let configured = config::configs::get().tower_compose_episode.iter()
        .find(|row| row.theme_id == context.theme_id && row.layer_id == context.layer_id)
        .ok_or(AppError::InvalidRequest)?.plane;
    if configured != 2 { return Ok(()); }
    let team = team_from_active(active, context)?;
    let mut tx = db.begin().await?;
    let state = tower_compose::get_theme_state_in_transaction(&mut tx, player_id, context.theme_id).await?;
    let boss = load_boss(&mut tx, player_id, context.theme_id, context.layer_id, state.as_ref()).await?;
    let both_fought = boss.planes.iter().all(|plane| plane.result.unwrap_or(0) != 0);
    let fresh_run = context.plane_id == 1 && both_fought && !boss.lock.unwrap_or(false);
    if context.plane_id == 2 && !boss.lock.unwrap_or(false)
        && !boss.planes.iter().any(|plane| plane.plane_id == Some(1) && plane.result == Some(1))
    {
        return Err(AppError::InvalidRequest);
    }
    if boss.lock.unwrap_or(false) {
        let expected = boss.planes.iter().find(|plane| plane.plane_id == Some(context.plane_id) && !plane.lock.unwrap_or(false))
            .and_then(|plane| plane.team.as_ref()).ok_or(AppError::InvalidRequest)?;
        if expected.heroes != team.heroes || expected.support_id != team.support_id
            || expected.research_id != team.research_id || expected.player_skill != team.player_skill
            || expected.support_assist_hero.as_ref().map(|hero| (hero.uid, hero.hero_id))
                != team.support_assist_hero.as_ref().map(|hero| (hero.uid, hero.hero_id))
        {
            return Err(AppError::InvalidRequest);
        }
    }
    if !fresh_run {
        let chosen = team_hero_ids(&team)?;
        for plane in boss.planes.iter().filter(|plane| plane.plane_id != Some(context.plane_id) && plane.result.unwrap_or(0) != 0) {
            let previous = team_hero_ids(plane.team.as_ref().ok_or(AppError::InvalidRequest)?)?;
            if !chosen.is_disjoint(&previous) { return Err(AppError::InvalidRequest); }
        }
    }
    tx.commit().await?;
    Ok(())
}

fn plane_score(
    theme_id: i32, team: &TowerComposeTeam, mods: &[TowerComposeMods], battle_id: i32, round: i32,
) -> Result<i32, AppError> {
    let tables = config::configs::get();
    let ignore = tables.tower_compose_const.get(12).map(|row| row.value.as_str()).unwrap_or("")
        .split('#').filter(|part| !part.is_empty()).map(str::parse::<i32>)
        .collect::<Result<HashSet<_>, _>>().map_err(|_| AppError::InvalidRequest)?;
    let max_level = tables.tower_compose_point_level.iter().map(|row| row.level).max().ok_or(AppError::InvalidRequest)?;
    let mut base = 0_i64;
    for selected in mods.iter().flat_map(|category| &category.mods) {
        let id = selected.mod_id.unwrap_or_default();
        if id == 0 || ignore.contains(&id) { continue; }
        let item = tables.tower_compose_mod.get(id).filter(|item| item.theme_id == theme_id).ok_or(AppError::InvalidRequest)?;
        let level = item.level.min(max_level);
        let points = tables.tower_compose_point_level.iter().find(|row| row.level == level).ok_or(AppError::InvalidRequest)?;
        base += i64::from(points.boss_point_base);
    }
    let mut hero_points = Vec::new();
    for hero in &team.heroes {
        let trial = hero.trial_id.unwrap_or_default();
        let id = if trial > 0 { tables.hero_trial.get(trial).ok_or(AppError::InvalidRequest)?.hero_id }
            else { hero.hero_id.unwrap_or_default() };
        if let Some(extra) = tables.tower_compose_extra.iter().find(|extra| extra.theme_id == theme_id && extra.id == id) {
            hero_points.push(extra.boss_point_base);
        }
    }
    hero_points.sort_unstable_by(|a, b| b.cmp(a));
    base += hero_points.into_iter().take(2).map(i64::from).sum::<i64>();
    let max_round = tables.battle.get(battle_id).ok_or(AppError::InvalidRequest)?.max_round;
    let remaining = max_round.saturating_sub(round.max(0)).max(0);
    let bonus = tables.tower_compose_point_round.iter().filter(|row| row.round <= remaining)
        .max_by_key(|row| row.round).ok_or(AppError::InvalidRequest)?.boss_point_add;
    i32::try_from(base.saturating_mul(1000_i64 + i64::from(bonus)) / 1000)
        .map_err(|_| AppError::InvalidRequest)
}

pub(super) async fn settle(
    tx: &mut Transaction<'_, Sqlite>, player_id: i64, active: &ActiveBattle, context: ComposeBattle,
) -> Result<TowerComposeFightSettlePush, AppError> {
    let tables = config::configs::get();
    let max_plane = tables.tower_compose_episode.iter()
        .find(|row| row.theme_id == context.theme_id && row.layer_id == context.layer_id)
        .ok_or(AppError::InvalidRequest)?.plane;
    let state = tower_compose::get_theme_state_in_transaction(tx, player_id, context.theme_id).await?;
    let mut boss = load_boss(tx, player_id, context.theme_id, context.layer_id, state.as_ref()).await?;
    let old_high = state.as_ref().map(|value| value.high_score).unwrap_or_default();
    let previous_current = boss.planes.iter().find(|plane| plane.plane_id == Some(context.plane_id))
        .and_then(|plane| plane.curr_score).unwrap_or_default();
    let saved_layer = sqlx::query_scalar::<_, i32>("SELECT saved_layer_id FROM user_tower_compose_boss_records WHERE user_id = ? AND theme_id = ?")
        .bind(player_id).bind(context.theme_id).fetch_optional(&mut **tx).await?.unwrap_or_default();
    let previous_saved = if saved_layer == context.layer_id {
        saved_record(tx, player_id, context.theme_id).await?
            .and_then(|record| record.boss)
            .and_then(|boss| boss.planes.into_iter().find(|plane| plane.plane_id == Some(context.plane_id)))
            .and_then(|plane| plane.curr_score).unwrap_or_default()
    } else { 0 };
    let previous_score = previous_current.max(previous_saved);
    let both_fought = boss.planes.iter().all(|plane| plane.result.unwrap_or(0) != 0);
    if max_plane == 2 && context.plane_id == 1 && both_fought && !boss.lock.unwrap_or(false) {
        sqlx::query("DELETE FROM user_tower_compose_boss_planes WHERE user_id = ? AND theme_id = ? AND layer_id = ?")
            .bind(player_id).bind(context.theme_id).bind(context.layer_id).execute(&mut **tx).await?;
        boss = load_boss(tx, player_id, context.theme_id, context.layer_id, state.as_ref()).await?;
    }
    let team = team_from_active(active, context)?;
    let plane = boss.planes.iter_mut().find(|plane| plane.plane_id == Some(context.plane_id)).ok_or(AppError::InvalidRequest)?;
    if let Some(data) = &active.compose_data {
        if data.theme_id != context.theme_id || data.layer_id != context.layer_id || data.plane_id != context.plane_id {
            return Err(AppError::InvalidRequest);
        }
        plane.mods = data.mods.clone();
    }
    let won = active.runtime.outcome() == battle::BattleOutcome::Victory;
    let scored = if won { plane_score(context.theme_id, &team, &plane.mods, active.battle_id, active.current_round())? } else { 0 };
    let retry = boss.lock.unwrap_or(false);
    let score = if retry && won { scored.max(previous_score) } else { scored };
    plane.team = Some(team);
    plane.curr_score = Some(score);
    plane.result = Some(if won { 1 } else { 2 });
    let now = common::time::ServerTime::now_ms();
    sqlx::query("INSERT INTO user_tower_compose_boss_planes (user_id, theme_id, layer_id, plane_id, plane_json, updated_at) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(user_id, theme_id, layer_id, plane_id) DO UPDATE SET plane_json = excluded.plane_json, updated_at = excluded.updated_at")
        .bind(player_id).bind(context.theme_id).bind(context.layer_id).bind(context.plane_id)
        .bind(serde_json::to_string(plane)?).bind(now).execute(&mut **tx).await?;
    if retry {
        boss.lock = Some(false);
        for plane in &mut boss.planes {
            plane.lock = Some(false);
            if plane.result.unwrap_or_default() != 0 { write_plane(tx, player_id, context.theme_id, context.layer_id, plane).await?; }
        }
        sqlx::query("UPDATE user_tower_compose_theme_state SET boss_lock = 0 WHERE user_id = ? AND theme_id = ?")
            .bind(player_id).bind(context.theme_id).execute(&mut **tx).await?;
        sqlx::query("UPDATE user_tower_compose_boss_records SET resume_layer_id = 0, resume_boss_json = '' WHERE user_id = ? AND theme_id = ?")
            .bind(player_id).bind(context.theme_id).execute(&mut **tx).await?;
    }
    let total = boss.planes.iter().map(|plane| plane.curr_score.unwrap_or_default()).sum::<i32>();
    let completed = (1..=max_plane).all(|id| boss.planes.iter().any(|plane| plane.plane_id == Some(id) && plane.result == Some(1)));
    let fought_mods = boss.planes.iter().filter(|plane| plane.plane_id.is_some_and(|id| (1..=max_plane).contains(&id)))
        .map(|plane| TowerComposePlaneMods { plane_id: plane.plane_id, mods: plane.mods.clone() }).collect::<Vec<_>>();
    boss.level = Some(max_selected_mod_level(&fought_mods));
    if completed {
        tower_compose::complete_layer_in_transaction(tx, player_id, context.theme_id, context.layer_id).await?;
    }
    sqlx::query("INSERT INTO user_tower_compose_theme_state (user_id, theme_id, high_score, curr_score, boss_level, updated_at) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(user_id, theme_id) DO UPDATE SET high_score = MAX(high_score, excluded.high_score), curr_score = excluded.curr_score, boss_level = excluded.boss_level, updated_at = excluded.updated_at")
        .bind(player_id).bind(context.theme_id).bind(if completed { total } else { 0 }).bind(total).bind(boss.level.unwrap_or_default()).bind(now)
        .execute(&mut **tx).await?;
    let task_outcome = database::db::game::tasks::TowerComposeTaskOutcome {
        theme_id: context.theme_id, layer_id: context.layer_id, plane_id: context.plane_id,
        won, completed, boss_level: boss.level.unwrap_or_default(), score: total,
        planes: boss.planes.clone(),
    };
    database::db::game::tasks::sync_tower_compose_tasks_in_transaction(tx, player_id, Some(&task_outcome)).await?;
    let after = tower_compose::get_theme_state_in_transaction(tx, player_id, context.theme_id).await?;
    boss.high_score = Some(after.as_ref().map(|value| value.high_score).unwrap_or_default());
    boss.curr_score = Some(total);
    let final_result = max_plane == 1 || context.plane_id == max_plane || !won || retry;
    let record = final_result.then(|| TowerComposeBossRecord { create_time: Some(now), boss: Some(boss.clone()) });
    let record_json = record.as_ref().map(serde_json::to_string).transpose()?.unwrap_or_default();
    sqlx::query("INSERT INTO user_tower_compose_boss_records (user_id, theme_id, layer_id, current_record_json, updated_at) VALUES (?, ?, ?, ?, ?) ON CONFLICT(user_id, theme_id) DO UPDATE SET layer_id = excluded.layer_id, current_record_json = excluded.current_record_json, updated_at = excluded.updated_at")
        .bind(player_id).bind(context.theme_id).bind(context.layer_id).bind(record_json).bind(now).execute(&mut **tx).await?;
    let mut theme = theme_info(context.theme_id, after.as_ref());
    theme.boss = Some(boss.clone());
    theme.curr_record = saved_record(tx, player_id, context.theme_id).await?;
    theme.saved_record = Some(theme.curr_record.is_some());
    let planes = boss.planes.iter().filter(|plane| plane.result.unwrap_or_default() != 0).map(|plane| TowerComposePlaneSettle {
        plane_id: plane.plane_id, score: plane.curr_score,
        new_flag: Some(plane.plane_id == Some(context.plane_id) && scored > previous_score), result: plane.result,
    }).collect();
    Ok(TowerComposeFightSettlePush {
        theme: Some(theme), boss_settle: Some(TowerComposeBossSettle { planes, new_flag: Some(completed && total > old_high), record }),
        result: Some(if won { 1 } else { 2 }), params: active.params.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;
    use sonettobuf::{Fight, FightEntityInfo, FightGroup, FightTeam, TowerComposeMod};

    async fn fixture() -> SqlitePool {
        let path = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        config::init(&path).unwrap();
        let db = SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&db).await.unwrap();
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (9, 'isolated-compose-boss', 0, 0)")
            .execute(&db).await.unwrap();
        sqlx::query("INSERT INTO user_tower_compose_theme_state (user_id, theme_id, pass_max_layer_id, updated_at) VALUES (9, 1, 5, 0)")
            .execute(&db).await.unwrap();
        db
    }

    fn mods(first: i32, second: i32) -> Vec<TowerComposePlaneMods> {
        [first, second].into_iter().enumerate().map(|(index, mod_id)| TowerComposePlaneMods {
            plane_id: Some(index as i32 + 1),
            mods: vec![TowerComposeMods { r#type: Some(1), mods: vec![TowerComposeMod { slot: Some(1), mod_id: Some(mod_id) }] }],
        }).collect()
    }

    fn active(plane: i32, uid: i64, hero_id: i32, victory: bool) -> ActiveBattle {
        let entity = |uid, model_id, position, hp| FightEntityInfo {
            uid: Some(uid), model_id: Some(model_id), position: Some(position), current_hp: Some(hp),
            ..Default::default()
        };
        let fight = Fight {
            battle_id: Some(9001001),
            cur_wave: Some(2),
            attacker: Some(FightTeam { entitys: vec![entity(uid, hero_id, 1, if victory { 100 } else { 0 })], ..Default::default() }),
            defender: Some(FightTeam { entitys: vec![entity(-1, 1, 1, if victory { 0 } else { 100 })], ..Default::default() }),
            ..Default::default()
        };
        ActiveBattle {
            chapter_id: 9040, episode_id: 90400199, battle_id: 9001001,
            params: Some(serde_json::json!({"themeId":1,"layerId":6,"planeId":plane,"supportId":0,"researchId":0,"supportAssistUid":0}).to_string()),
            fight_group: Some(FightGroup { hero_list: vec![uid], cloth_id: Some(7), ..Default::default() }),
            runtime: battle::tooling::unstarted_battle(battle::catalog::BattleCatalog::new(config::configs::get()), fight),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn no_saved_mods_uses_the_configured_default_environment() {
        let db = fixture().await;
        let mut tx = db.begin().await.unwrap();
        let state = tower_compose::get_theme_state_in_transaction(&mut tx, 9, 1).await.unwrap();
        let boss = load_boss(&mut tx, 9, 1, 6, state.as_ref()).await.unwrap();
        assert_eq!(boss.level, Some(5));
        assert_eq!(boss.planes.len(), 2);
        for plane in boss.planes {
            assert!(plane.mods.iter().any(|group| group.r#type == Some(3)
                && group.mods.iter().any(|item| item.slot == Some(1) && item.mod_id == Some(11601))));
        }
    }

    #[tokio::test]
    async fn single_plane_boss_does_not_offer_an_unsendable_second_selection() {
        let db = fixture().await;
        let mut tx = db.begin().await.unwrap();
        let state = tower_compose::get_theme_state_in_transaction(&mut tx, 9, 1).await.unwrap();
        let single = load_boss(&mut tx, 9, 1, 3, state.as_ref()).await.unwrap();
        assert_eq!(single.planes[0].lock, Some(false));
        assert_eq!(single.planes[1].lock, Some(true));
        let double = load_boss(&mut tx, 9, 1, 6, state.as_ref()).await.unwrap();
        assert_eq!(double.planes[0].lock, Some(false));
        assert_eq!(double.planes[1].lock, Some(false));
    }

    #[test]
    fn score_uses_real_mod_hero_and_remaining_round_tables() {
        let path = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        config::init(&path).unwrap();
        let mut team = empty_team();
        team.heroes = [3041, 3034, 3127].into_iter().enumerate().map(|(pos, id)| TowerComposeHero {
            pos: Some(pos as i32 + 1), hero_id: Some(id), trial_id: Some(0), mind_id: Some(0),
        }).collect();
        // 100 from the mod + 200 and 100 from the two best bonus heroes;
        // 15 remaining rounds add 270 per mille: floor(400 * 1.27) = 508.
        assert_eq!(plane_score(1, &team, &mods(11101, 11102)[0].mods, 9001001, 5).unwrap(), 508);
    }

    #[tokio::test]
    async fn two_plane_settlement_persists_scores_locks_teams_and_saves_records() {
        let db = fixture().await;
        super::super::set_mods(&db, 9, 1, mods(11101, 11102)).await.unwrap();
        let first = active(1, 101, 3041, true);
        assert!(compose_battle(&first).is_some());
        ensure_team_available(&db, 9, &first).await.unwrap();
        assert!(ensure_team_available(&db, 9, &active(2, 102, 3034, true)).await.is_err());

        let mut tx = db.begin().await.unwrap();
        let first_push = super::super::settle_in_transaction(&mut tx, 9, &first).await.unwrap().unwrap();
        assert_eq!(first_push.boss_settle.as_ref().unwrap().planes.len(), 1);
        assert!(first_push.boss_settle.as_ref().unwrap().record.is_none());
        tx.rollback().await.unwrap();
        assert_eq!(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM user_tower_compose_boss_planes").fetch_one(&db).await.unwrap(), 0);

        let mut tx = db.begin().await.unwrap();
        let first_push = super::super::settle_in_transaction(&mut tx, 9, &first).await.unwrap().unwrap();
        let first_score = first_push.boss_settle.unwrap().planes[0].score.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(first_score, 262);
        let state = tower_compose::get_theme_state(&db, 9, 1).await.unwrap().unwrap();
        assert_eq!(state.pass_max_layer_id, 5);
        assert_eq!(state.high_score, 0, "a single plane is not a completed two-plane run");
        let duplicate = active(2, 102, 3041, true);
        assert!(matches!(ensure_team_available(&db, 9, &duplicate).await, Err(AppError::InvalidRequest)));
        let second = active(2, 102, 3034, true);
        ensure_team_available(&db, 9, &second).await.unwrap();
        let mut tx = db.begin().await.unwrap();
        let push = super::super::settle_in_transaction(&mut tx, 9, &second).await.unwrap().unwrap();
        let settlement = push.boss_settle.unwrap();
        assert_eq!(settlement.planes.len(), 2);
        assert_eq!(settlement.planes[1].score, Some(393));
        assert!(settlement.record.is_some());
        tx.commit().await.unwrap();
        let state = tower_compose::get_theme_state(&db, 9, 1).await.unwrap().unwrap();
        assert_eq!(state.pass_max_layer_id, 6);
        assert_eq!(state.high_score, 655);

        let saved = update_record(&db, 9, 1, true).await.unwrap().unwrap();
        assert_eq!(saved.boss.as_ref().unwrap().curr_score, Some(655));
        assert_eq!(get_saved_record(&db, 9, 1).await.unwrap(), Some(saved.clone()));
        let info = super::super::get_info(&db, 9).await.unwrap().info.unwrap();
        let theme = info.themes.into_iter().find(|theme| theme.theme_id == Some(1)).unwrap();
        assert_eq!(theme.curr_record, Some(saved.clone()));
        assert_eq!(theme.boss.unwrap().curr_score, Some(655));

        let replay = re_challenge(&db, 9, 1, 1).await.unwrap();
        assert_eq!(replay.lock, Some(true));
        assert_eq!(replay.planes[0].lock, Some(false));
        assert_eq!(replay.planes[1].lock, Some(true));
        assert!(validate_mods(&db, 9, 1, &mods(11101, 11103)).await.is_err());
        let first_only = vec![mods(11101, 11102).remove(0)];
        let reply = super::super::set_mods(&db, 9, 1, first_only).await.unwrap();
        let locked = reply.planes.iter().find(|plane| plane.plane_id == Some(2)).unwrap();
        assert_eq!(locked.lock, Some(true));
        assert_eq!(selected_mods(&locked.mods), selected_mods(&mods(11101, 11102)[1].mods));
        ensure_team_available(&db, 9, &first).await.unwrap();
        let restored = cancel_re_challenge(&db, 9, 1).await.unwrap();
        assert_eq!(restored.lock, Some(false));
        assert_eq!(restored.curr_score, Some(655));
        assert_eq!(get_saved_record(&db, 9, 1).await.unwrap(), Some(saved.clone()));
        re_challenge(&db, 9, 1, 1).await.unwrap();
        let mut slower_retry = active(1, 101, 3041, true);
        let (mut fight, _) = slower_retry.runtime.reconnect_state();
        fight.cur_round = Some(19);
        slower_retry.runtime = battle::tooling::unstarted_battle(
            battle::catalog::BattleCatalog::new(config::configs::get()), fight,
        );
        let mut tx = db.begin().await.unwrap();
        let retry = super::super::settle_in_transaction(&mut tx, 9, &slower_retry).await.unwrap().unwrap();
        let retry = retry.boss_settle.unwrap();
        assert_eq!(retry.planes.len(), 2);
        assert!(retry.record.is_some());
        assert_eq!(retry.planes[0].score, Some(262));
        assert_eq!(retry.planes[0].new_flag, Some(false));
        tx.commit().await.unwrap();
        assert!(!tower_compose::get_theme_state(&db, 9, 1).await.unwrap().unwrap().boss_lock);
        assert_eq!(get_saved_record(&db, 9, 1).await.unwrap(), Some(saved));
        let fresh = active(1, 103, 3127, true);
        ensure_team_available(&db, 9, &fresh).await.unwrap();
        let mut tx = db.begin().await.unwrap();
        let next = super::super::settle_in_transaction(&mut tx, 9, &fresh).await.unwrap().unwrap().boss_settle.unwrap();
        assert_eq!(next.planes[0].score, Some(196));
        assert_eq!(next.planes[0].new_flag, Some(false));
        assert_eq!(next.new_flag, Some(false));
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn failed_second_plane_does_not_complete_layer_or_award_score() {
        let db = fixture().await;
        super::super::set_mods(&db, 9, 1, mods(11101, 11102)).await.unwrap();
        let mut tx = db.begin().await.unwrap();
        super::super::settle_in_transaction(&mut tx, 9, &active(1, 101, 3041, true)).await.unwrap();
        tx.commit().await.unwrap();
        let mut tx = db.begin().await.unwrap();
        let reply = super::super::settle_in_transaction(&mut tx, 9, &active(2, 102, 3034, false)).await.unwrap().unwrap();
        assert_eq!(reply.result, Some(2));
        let boss = reply.boss_settle.unwrap();
        assert_eq!(boss.planes[1].result, Some(2));
        assert_eq!(boss.planes[1].score, Some(0));
        assert!(boss.record.is_some());
        tx.commit().await.unwrap();
        let state = tower_compose::get_theme_state(&db, 9, 1).await.unwrap().unwrap();
        assert_eq!(state.pass_max_layer_id, 5);
        assert_eq!(state.high_score, 0);
    }

    #[tokio::test]
    async fn first_plane_end_handler_sends_settlement_before_fight_end() {
        use crate::{handlers::dungeon::on_fight_end_fight,
            net::{app::AppState, context::ConnectionContext, outbound::CommandPacket, packet::ClientPacket},
            player::{Player, PlayerState}};
        use database::db::game::battle::{create_fight_instance, NewFightInstance};
        use prost::Message;
        use sonettobuf::{CmdId, EndFightRequest};

        let db = fixture().await;
        super::super::set_mods(&db, 9, 1, mods(11101, 11102)).await.unwrap();
        let heroes = database::models::game::heros::UserHeroModel::new(9, db.clone());
        let uid = heroes.create_hero(3041).await.unwrap();
        let fight_id = create_fight_instance(&db, NewFightInstance {
            user_id: 9, episode_id: 90400199, battle_id: 9001001, multiplication: 1,
            entry_cost: "{}", checkpoint: "", created_at: 0,
        }).await.unwrap();
        let mut fight = active(1, uid, 3041, true);
        fight.fight_id = Some(fight_id);
        assert!(fight.is_victory());
        let state = Box::leak(Box::new(AppState::new(db, config::configs::get())));
        let (outbound, mut packets) = tokio::sync::mpsc::channel(32);
        let mut ctx = ConnectionContext::new(outbound, state);
        ctx.player = Some(Player::new(9, PlayerState::new(9, 0)));
        ctx.player_mut().unwrap().battle.restore_active(fight);
        on_fight_end_fight(&mut ctx, ClientPacket { sequence: 1, cmd_id: CmdId::FightEndFightCmd as i16,
            up_tag: 1, data: EndFightRequest { is_abort: Some(false) }.encode_to_vec() }).await.unwrap();
        let mut commands = Vec::new();
        let mut settlement = None;
        while let Ok(packet) = packets.try_recv() {
            match packet {
                CommandPacket::Push { cmd_id, body, .. } => {
                    if cmd_id == CmdId::TowerComposeFightSettlePushCmd {
                        settlement = Some(TowerComposeFightSettlePush::decode(&*body).unwrap());
                    }
                    commands.push(cmd_id);
                }
                CommandPacket::Reply { cmd_id, result_code, .. } => { assert_eq!(result_code, 0); commands.push(cmd_id); }
            }
        }
        let settlement = settlement.expect("missing compose settlement").boss_settle.unwrap();
        assert_eq!(settlement.planes.len(), 1);
        assert_eq!(settlement.planes[0].plane_id, Some(1));
        let position = |cmd| commands.iter().position(|item| *item == cmd).unwrap();
        assert!(position(CmdId::TowerComposeFightSettlePushCmd) < position(CmdId::FightEndFightPushCmd));
        assert!(position(CmdId::FightEndFightPushCmd) < position(CmdId::FightEndFightCmd));
        assert!(!ctx.player().unwrap().battle.has_active());
    }
}
