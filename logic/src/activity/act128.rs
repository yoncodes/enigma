use crate::error::AppError;
use common::time::ServerTime;
use database::db::game::activity_state::{self, ActivityStateKind};
use sonettobuf::{Act128BossDetail, Get128InfosReply};
use sqlx::{Sqlite, SqlitePool, Transaction};

pub async fn act128_info(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
) -> Result<Get128InfosReply, AppError> {
    let tables = config::configs::get();
    let activity_id = activity_id
        .or_else(|| tables.latest_open_activity_id(128))
        .ok_or(AppError::InvalidRequest)?;
    let saved = activity_state::get(
        db,
        player_id,
        activity_id,
        ActivityStateKind::Act128BossScore,
    )
    .await?;
    let mut boss_ids = tables
        .activity128_episode
        .iter()
        .filter(|row| row.activity_id == activity_id)
        .map(|row| row.stage)
        .collect::<Vec<_>>();
    boss_ids.sort_unstable();
    boss_ids.dedup();

    Ok(Get128InfosReply {
        activity_id: Some(activity_id),
        boss_detail: boss_ids
            .into_iter()
            .map(|boss_id| {
                let (total, highest, _) = saved.get(&boss_id).cloned().unwrap_or_default();
                Act128BossDetail {
                    boss_id: Some(boss_id),
                    total_point: Some(total),
                    highest_point: Some(highest),
                    double_num: Some(0),
                    layer4_total_point: Some(0),
                    layer4_highest_point: Some(0),
                    sp_highest_point: Some(0),
                    ..Default::default()
                }
            })
            .collect(),
        player_level: Some(0),
        player_exp: Some(0),
        gain_milestone_level: Some(0),
    })
}

pub async fn settle_act128_score_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    player_id: i64,
    episode_id: i32,
    battle_id: i32,
    score: i32,
) -> Result<(), AppError> {
    let Some(route) = config::configs::get().activity128_battle(episode_id, battle_id) else {
        return Ok(());
    };
    if score <= 0 {
        return Ok(());
    }

    sqlx::query(
        "INSERT INTO user_activity_state
            (user_id, activity_id, kind, entry_id, state, progress, ext, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, '', ?)
         ON CONFLICT(user_id, activity_id, kind, entry_id) DO UPDATE SET
            state = MIN(2147483647, user_activity_state.state + excluded.state),
            progress = MAX(user_activity_state.progress, excluded.progress),
            updated_at = excluded.updated_at",
    )
    .bind(player_id)
    .bind(route.activity_id)
    .bind(ActivityStateKind::Act128BossScore.id())
    .bind(route.boss_id)
    .bind(score)
    .bind(score)
    .bind(ServerTime::now_ms())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn score_settlement_accumulates_total_and_preserves_the_best_attempt() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (7, 'act128', 0, 0)",
        )
        .execute(&db)
        .await
        .unwrap();

        for score in [4_580_648, 100] {
            let mut tx = db.begin().await.unwrap();
            settle_act128_score_in_transaction(&mut tx, 7, 13500420, 118353100, score)
                .await
                .unwrap();
            tx.commit().await.unwrap();
        }

        let reply = act128_info(&db, 7, Some(138520)).await.unwrap();
        let boss = reply
            .boss_detail
            .iter()
            .find(|boss| boss.boss_id == Some(2))
            .unwrap();
        assert_eq!(boss.total_point, Some(4_580_748));
        assert_eq!(boss.highest_point, Some(4_580_648));
    }
}
