use crate::error::AppError;
use common::time::ServerTime;
use sonettobuf::{
    Act229BattleFinishPush, Act229HeroNo, Act229ModifySkillReply, Act229ResetStageReply,
    Act229StageNo, GetAct229InfoReply,
};
use sqlx::SqlitePool;

pub async fn act229_info(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
) -> Result<GetAct229InfoReply, AppError> {
    let tables = config::configs::get();
    let activity_id = activity_id
        .or_else(|| {
            tables
                .activity229_const
                .get(2)
                .and_then(|row| row.value.parse().ok())
        })
        .ok_or(AppError::InvalidRequest)?;
    let saved = sqlx::query_as::<_, (i32, i32, i32, i32, i32, String, i32)>(
        "SELECT stage_id, star, max_star, round, min_round, heroes_json, skill_id
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
                hero_group_sub_id: None,
                skill_ids: state
                    .and_then(|state| (state.6 != 0).then_some(vec![state.6]))
                    .unwrap_or_default(),
                last_update_team_time: None,
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
