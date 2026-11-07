use crate::error::AppError;
use crate::types::hero_group_snapshot_type::HeroGroupSnapshotType;
use common::time::ServerTime;
use sonettobuf::{
    Act229BattleFinishPush, Act229HeroNo, Act229ModifySkillReply, Act229ModifyStageSubIdReply,
    Act229ResetStageReply, Act229StageNo, Act229StageTeamSubIdNo, Act229StageTeamSubIdPush,
    GetAct229InfoReply,
};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::{HashMap, HashSet};

fn current_activity_id() -> Result<i32, AppError> {
    config::configs::get()
        .activity229_const
        .get(2)
        .and_then(|row| row.value.parse().ok())
        .ok_or(AppError::InvalidRequest)
}

pub async fn act229_info(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
) -> Result<GetAct229InfoReply, AppError> {
    let tables = config::configs::get();
    let activity_id = activity_id.map_or_else(current_activity_id, Ok)?;
    let saved = sqlx::query_as::<_, (i32, i32, i32, i32, i32, String, i32, i32, i64)>(
        "SELECT stage_id, star, max_star, round, min_round, heroes_json, skill_id,
                hero_group_sub_id, last_update_team_time
         FROM user_activity229_stages
         WHERE user_id = ? AND activity_id = ?",
    )
    .bind(player_id)
    .bind(activity_id)
    .fetch_all(db)
    .await?;

    let mut stages = tables
        .activity229_episode
        .iter()
        .filter(|row| row.activity_id == activity_id)
        .map(|row| {
            let state = saved.iter().find(|state| state.0 == row.stage);
            let heroes = state
                .map(|state| heroes(&state.5))
                .transpose()?
                .unwrap_or_default();

            Ok(Act229StageNo {
                stage_id: Some(row.stage),
                star: Some(state.map(|state| state.1).unwrap_or_default()),
                max_star: Some(state.map(|state| state.2).unwrap_or_default()),
                round: Some(state.map(|state| state.3).unwrap_or_default()),
                min_round: Some(state.map(|state| state.4).unwrap_or_default()),
                heros: heroes,
                hero_group_sub_id: Some(
                    state
                        .map(|state| state.7)
                        .filter(|sub_id| *sub_id > 0)
                        .unwrap_or(row.stage),
                ),
                skill_ids: state
                    .and_then(|state| (state.6 != 0).then_some(vec![state.6]))
                    .unwrap_or_default(),
                last_update_team_time: state
                    .map(|state| state.8)
                    .filter(|updated_at| *updated_at > 0),
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    stages.sort_by_key(|stage| stage.stage_id.unwrap_or_default());

    Ok(GetAct229InfoReply {
        activity_id: Some(activity_id),
        stages,
    })
}

fn heroes(json: &str) -> Result<Vec<Act229HeroNo>, AppError> {
    Ok(serde_json::from_str(json)?)
}

pub fn act229_battle_episode(activity_id: i32, stage_id: i32) -> Result<i32, AppError> {
    config::configs::get()
        .activity229_episode
        .iter()
        .find(|row| row.activity_id == activity_id && row.stage == stage_id)
        .map(|row| row.episode_id)
        .ok_or(AppError::InvalidRequest)
}

pub async fn modify_act229_skill(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
    stage_id: i32,
    skill_ids: Vec<i32>,
) -> Result<Act229ModifySkillReply, AppError> {
    let [skill_id] = skill_ids.as_slice() else {
        return Err(AppError::InvalidRequest);
    };
    let tables = config::configs::get();
    let stage = tables
        .activity229_episode
        .iter()
        .find(|row| row.activity_id == activity_id && row.stage == stage_id)
        .ok_or(AppError::InvalidRequest)?;
    let allowed = stage
        .optional_skills
        .split('#')
        .map(|id| id.parse::<i32>().map_err(|_| AppError::InvalidRequest))
        .collect::<Result<Vec<_>, _>>()?;
    if !allowed.contains(skill_id) || tables.activity229_skill.get(*skill_id).is_none() {
        return Err(AppError::InvalidRequest);
    }

    let saved_heroes = sqlx::query_scalar::<_, String>(
        "SELECT heroes_json
         FROM user_activity229_stages
         WHERE user_id = ? AND activity_id = ? AND stage_id = ?",
    )
    .bind(player_id)
    .bind(activity_id)
    .bind(stage_id)
    .fetch_optional(db)
    .await?;
    if saved_heroes
        .as_deref()
        .map(heroes)
        .transpose()?
        .is_some_and(|heroes| !heroes.is_empty())
    {
        return Err(AppError::InvalidRequest);
    }

    sqlx::query(
        "INSERT INTO user_activity229_stages
            (user_id, activity_id, stage_id, skill_id, updated_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(user_id, activity_id, stage_id) DO UPDATE SET
            skill_id = excluded.skill_id,
            updated_at = excluded.updated_at",
    )
    .bind(player_id)
    .bind(activity_id)
    .bind(stage_id)
    .bind(skill_id)
    .bind(ServerTime::now_ms())
    .execute(db)
    .await?;

    Ok(Act229ModifySkillReply {
        activity_id: Some(activity_id),
        stage_id: Some(stage_id),
        skill_ids,
    })
}

pub async fn modify_act229_stage_sub_ids(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
    stage_sub_ids: Vec<Act229StageTeamSubIdNo>,
) -> Result<Act229ModifyStageSubIdReply, AppError> {
    if stage_sub_ids.is_empty() {
        return Err(AppError::InvalidRequest);
    }
    let configured = config::configs::get()
        .activity229_episode
        .iter()
        .filter(|row| row.activity_id == activity_id)
        .map(|row| row.stage)
        .collect::<HashSet<_>>();
    if configured.is_empty() {
        return Err(AppError::InvalidRequest);
    }
    let saved = sqlx::query_as::<_, (i32, String, i32, i64)>(
        "SELECT stage_id, heroes_json, hero_group_sub_id, last_update_team_time
         FROM user_activity229_stages
         WHERE user_id = ? AND activity_id = ?",
    )
    .bind(player_id)
    .bind(activity_id)
    .fetch_all(db)
    .await?;
    let saved = saved
        .into_iter()
        .map(|(stage_id, heroes, sub_id, updated_at)| {
            Ok((
                stage_id,
                (
                    if sub_id > 0 { sub_id } else { stage_id },
                    updated_at,
                    !self::heroes(&heroes)?.is_empty(),
                ),
            ))
        })
        .collect::<Result<HashMap<_, _>, AppError>>()?;
    let mut mappings = configured
        .iter()
        .map(|stage_id| {
            let state = saved
                .get(stage_id)
                .copied()
                .unwrap_or((*stage_id, 0, false));
            (*stage_id, state)
        })
        .collect::<HashMap<_, _>>();
    let original_times = mappings
        .values()
        .map(|(sub_id, updated_at, _)| (*sub_id, *updated_at))
        .collect::<HashMap<_, _>>();
    if original_times.len() != mappings.len() {
        return Err(AppError::InvalidRequest);
    }
    let selected = &stage_sub_ids[0];
    let selected_stage_id = selected.stage_id.ok_or(AppError::InvalidRequest)?;
    let selected_sub_id = selected.hero_group_sub_id.ok_or(AppError::InvalidRequest)?;
    let selected_state = mappings
        .get(&selected_stage_id)
        .copied()
        .ok_or(AppError::InvalidRequest)?;
    if selected_sub_id <= 0 || selected_state.0 == selected_sub_id || selected_state.2 {
        return Err(AppError::InvalidRequest);
    }
    let displaced_stage = mappings.iter().find_map(|(stage_id, (sub_id, _, _))| {
        (*stage_id != selected_stage_id && *sub_id == selected_sub_id).then_some(*stage_id)
    });
    let mut expected = mappings
        .iter()
        .map(|(stage_id, (sub_id, _, _))| (*stage_id, *sub_id))
        .collect::<HashMap<_, _>>();
    expected.insert(selected_stage_id, selected_sub_id);
    if let Some(displaced_stage) = displaced_stage {
        if stage_sub_ids.len() != mappings.len() {
            return Err(AppError::InvalidRequest);
        }
        expected.insert(displaced_stage, selected_state.0);
    } else if stage_sub_ids.len() != 1 {
        return Err(AppError::InvalidRequest);
    }

    let mut requested = HashSet::new();
    for stage in &stage_sub_ids {
        let stage_id = stage.stage_id.ok_or(AppError::InvalidRequest)?;
        let sub_id = stage.hero_group_sub_id.ok_or(AppError::InvalidRequest)?;
        if !requested.insert(stage_id) || expected.get(&stage_id) != Some(&sub_id) {
            return Err(AppError::InvalidRequest);
        }
        let state = mappings
            .get_mut(&stage_id)
            .ok_or(AppError::InvalidRequest)?;
        state.0 = sub_id;
        state.1 = original_times.get(&sub_id).copied().unwrap_or_default();
    }
    let unique_groups = mappings
        .values()
        .map(|(sub_id, _, _)| *sub_id)
        .collect::<HashSet<_>>();
    if unique_groups.len() != mappings.len() {
        return Err(AppError::InvalidRequest);
    }

    let mut tx = db.begin().await?;
    for stage_id in requested {
        let (sub_id, updated_at, _) = mappings[&stage_id];
        sqlx::query(
            "INSERT INTO user_activity229_stages
                (user_id, activity_id, stage_id, hero_group_sub_id, last_update_team_time,
                 updated_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(user_id, activity_id, stage_id) DO UPDATE SET
                hero_group_sub_id = excluded.hero_group_sub_id,
                last_update_team_time = excluded.last_update_team_time,
                updated_at = excluded.updated_at",
        )
        .bind(player_id)
        .bind(activity_id)
        .bind(stage_id)
        .bind(sub_id)
        .bind(updated_at)
        .bind(ServerTime::now_ms())
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    Ok(Act229ModifyStageSubIdReply {
        activity_id: Some(activity_id),
        stages: act229_info(db, player_id, Some(activity_id)).await?.stages,
    })
}

pub async fn touch_act229_stage_teams(
    tx: &mut Transaction<'_, Sqlite>,
    player_id: i64,
    snapshot_id: i32,
    sub_ids: &[i32],
) -> Result<Option<Act229StageTeamSubIdPush>, AppError> {
    if snapshot_id != HeroGroupSnapshotType::Abyss.id() || sub_ids.is_empty() {
        return Ok(None);
    }
    let activity_id = current_activity_id()?;
    let saved = sqlx::query_as::<_, (i32, i32)>(
        "SELECT stage_id, hero_group_sub_id
         FROM user_activity229_stages
         WHERE user_id = ? AND activity_id = ?",
    )
    .bind(player_id)
    .bind(activity_id)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .collect::<HashMap<_, _>>();
    let sub_ids = sub_ids.iter().copied().collect::<HashSet<_>>();
    let updated_at = ServerTime::now_ms();
    let mut stages = config::configs::get()
        .activity229_episode
        .iter()
        .filter(|row| row.activity_id == activity_id)
        .filter_map(|row| {
            let sub_id = saved
                .get(&row.stage)
                .copied()
                .filter(|sub_id| *sub_id > 0)
                .unwrap_or(row.stage);
            sub_ids.contains(&sub_id).then_some((row.stage, sub_id))
        })
        .collect::<Vec<_>>();
    if stages.is_empty() {
        return Ok(None);
    }
    stages.sort_unstable();

    for (stage_id, sub_id) in &stages {
        sqlx::query(
            "INSERT INTO user_activity229_stages
                (user_id, activity_id, stage_id, hero_group_sub_id, last_update_team_time,
                 updated_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(user_id, activity_id, stage_id) DO UPDATE SET
                hero_group_sub_id = excluded.hero_group_sub_id,
                last_update_team_time = excluded.last_update_team_time,
                updated_at = excluded.updated_at",
        )
        .bind(player_id)
        .bind(activity_id)
        .bind(stage_id)
        .bind(sub_id)
        .bind(updated_at)
        .bind(updated_at)
        .execute(&mut **tx)
        .await?;
    }

    Ok(Some(Act229StageTeamSubIdPush {
        activity_id: Some(activity_id),
        stage_sub_ids: stages
            .into_iter()
            .map(|(stage_id, hero_group_sub_id)| Act229StageTeamSubIdNo {
                stage_id: Some(stage_id),
                hero_group_sub_id: Some(hero_group_sub_id),
                last_update_team_time: Some(updated_at),
            })
            .collect(),
    }))
}

pub async fn act229_heroes_available(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
    stage_id: i32,
    heroes: &[Act229HeroNo],
) -> Result<(), AppError> {
    let saved = sqlx::query_as::<_, (i32, String)>(
        "SELECT stage_id, heroes_json
         FROM user_activity229_stages
         WHERE user_id = ? AND activity_id = ?",
    )
    .bind(player_id)
    .bind(activity_id)
    .fetch_all(db)
    .await?;
    let selected = heroes
        .iter()
        .filter_map(|hero| hero.hero_id)
        .collect::<std::collections::HashSet<_>>();
    for (saved_stage_id, json) in saved {
        let saved_heroes = self::heroes(&json)?;
        if saved_stage_id == stage_id {
            if !saved_heroes.is_empty() && saved_heroes != heroes {
                return Err(AppError::InvalidRequest);
            }
        } else if saved_heroes
            .iter()
            .filter_map(|hero| hero.hero_id)
            .any(|hero_id| selected.contains(&hero_id))
        {
            return Err(AppError::InvalidRequest);
        }
    }
    Ok(())
}

pub async fn finish_act229_battle(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
    stage_id: i32,
    round: i32,
    star: i32,
    heroes: &[Act229HeroNo],
) -> Result<Act229BattleFinishPush, AppError> {
    act229_battle_episode(activity_id, stage_id)?;
    let heroes_json = serde_json::to_string(heroes)?;
    let mut tx = db.begin().await?;
    let last_min_round = sqlx::query_scalar::<_, i32>(
        "SELECT min_round
         FROM user_activity229_stages
         WHERE user_id = ? AND activity_id = ? AND stage_id = ?",
    )
    .bind(player_id)
    .bind(activity_id)
    .bind(stage_id)
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or_default();
    sqlx::query(
        "INSERT INTO user_activity229_stages
            (user_id, activity_id, stage_id, star, max_star, round, min_round, heroes_json, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(user_id, activity_id, stage_id) DO UPDATE SET
            star = excluded.star,
            max_star = MAX(user_activity229_stages.max_star, excluded.max_star),
            round = excluded.round,
            min_round = CASE
                WHEN user_activity229_stages.min_round <= 0 THEN excluded.min_round
                ELSE MIN(user_activity229_stages.min_round, excluded.min_round)
            END,
            heroes_json = excluded.heroes_json,
            updated_at = excluded.updated_at",
    )
    .bind(player_id)
    .bind(activity_id)
    .bind(stage_id)
    .bind(star)
    .bind(star)
    .bind(round)
    .bind(round)
    .bind(heroes_json)
    .bind(ServerTime::now_ms())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(Act229BattleFinishPush {
        activity_id: Some(activity_id),
        stage_id: Some(stage_id),
        round: Some(round),
        star: Some(star),
        last_min_round: Some(last_min_round),
    })
}

pub async fn reset_act229_stage(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
    stage_id: i32,
) -> Result<Act229ResetStageReply, AppError> {
    act229_battle_episode(activity_id, stage_id)?;
    sqlx::query(
        "UPDATE user_activity229_stages
         SET star = 0, round = 0, heroes_json = '[]', updated_at = ?
         WHERE user_id = ? AND activity_id = ? AND stage_id = ?",
    )
    .bind(ServerTime::now_ms())
    .bind(player_id)
    .bind(activity_id)
    .bind(stage_id)
    .execute(db)
    .await?;

    Ok(Act229ResetStageReply {
        activity_id: Some(activity_id),
        stage_id: Some(stage_id),
    })
}

#[cfg(test)]
#[path = "act229/tests.rs"]
mod tests;
