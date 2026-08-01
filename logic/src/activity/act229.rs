use crate::error::AppError;
use common::time::ServerTime;
use sonettobuf::{
    Act229BattleFinishPush, Act229HeroNo, Act229ResetStageReply, Act229StageNo, GetAct229InfoReply,
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
    let saved = sqlx::query_as::<_, (i32, i32, i32, i32, i32, String)>(
        "SELECT stage_id, star, max_star, round, min_round, heroes_json
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
            let heroes = state.map(|state| heroes(&state.5)).unwrap_or_default();

            Act229StageNo {
                stage_id: Some(row.stage),
                star: Some(state.map(|state| state.1).unwrap_or_default()),
                max_star: Some(state.map(|state| state.2).unwrap_or_default()),
                round: Some(state.map(|state| state.3).unwrap_or_default()),
                min_round: Some(state.map(|state| state.4).unwrap_or_default()),
                heros: heroes,
            }
        })
        .collect::<Vec<_>>();
    stages.sort_by_key(|stage| stage.stage_id.unwrap_or_default());

    Ok(GetAct229InfoReply {
        activity_id: Some(activity_id),
        stages,
    })
}

fn heroes(json: &str) -> Vec<Act229HeroNo> {
    serde_json::from_str(json).unwrap_or_default()
}

pub fn act229_battle_episode(activity_id: i32, stage_id: i32) -> Result<i32, AppError> {
    config::configs::get()
        .activity229_episode
        .iter()
        .find(|row| row.activity_id == activity_id && row.stage == stage_id)
        .map(|row| row.episode_id)
        .ok_or(AppError::InvalidRequest)
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
        let saved_heroes = self::heroes(&json);
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
mod test {
    use super::*;

    #[tokio::test]
    async fn finish_preserves_best_result_and_reset_only_unlocks_the_team() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (7, 'act229', 0, 0)",
        )
        .execute(&db)
        .await
        .unwrap();
        let heroes = vec![Act229HeroNo {
            hero_id: Some(3143),
            equip_uids: vec![49],
        }];

        let first = finish_act229_battle(&db, 7, 138521, 1, 15, 2, &heroes)
            .await
            .unwrap();
        assert_eq!(first.last_min_round, Some(0));
        let better = finish_act229_battle(&db, 7, 138521, 1, 10, 3, &heroes)
            .await
            .unwrap();
        assert_eq!(better.last_min_round, Some(15));
        act229_heroes_available(&db, 7, 138521, 1, &heroes)
            .await
            .unwrap();
        assert!(
            act229_heroes_available(
                &db,
                7,
                138521,
                1,
                &[Act229HeroNo {
                    hero_id: Some(3149),
                    equip_uids: vec![50],
                }],
            )
            .await
            .is_err()
        );
        assert!(
            act229_heroes_available(&db, 7, 138521, 2, &heroes)
                .await
                .is_err()
        );

        reset_act229_stage(&db, 7, 138521, 1).await.unwrap();
        let stage = act229_info(&db, 7, Some(138521))
            .await
            .unwrap()
            .stages
            .into_iter()
            .find(|stage| stage.stage_id == Some(1))
            .unwrap();
        assert_eq!(stage.star, Some(0));
        assert_eq!(stage.max_star, Some(3));
        assert_eq!(stage.round, Some(0));
        assert_eq!(stage.min_round, Some(10));
        assert!(stage.heros.is_empty());
        act229_heroes_available(&db, 7, 138521, 2, &heroes)
            .await
            .unwrap();
    }
}
