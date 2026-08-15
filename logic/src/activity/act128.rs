use crate::{error::AppError, reward};
use common::time::ServerTime;
use database::db::game::{
    activity_state::{self, ActivityStateKind, ActivityStateSet},
    currencies,
};
use sonettobuf::{Act128BossDetail, Act128GetMilestoneBonusReply, Get128InfosReply};
use sqlx::{Sqlite, SqlitePool, Transaction};

pub type Act128MilestoneClaim = reward::RewardedReply<Act128GetMilestoneBonusReply>;

pub async fn act128_info(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
) -> Result<Get128InfosReply, AppError> {
    let tables = config::configs::get();
    let activity_id = activity_id.ok_or(AppError::InvalidRequest)?;
    ensure_act128_activity(activity_id)?;
    let saved = activity_state::get(
        db,
        player_id,
        activity_id,
        ActivityStateKind::Act128BossScore,
    )
    .await?;
    let milestone = activity_state::get(
        db,
        player_id,
        activity_id,
        ActivityStateKind::Act128Milestone,
    )
    .await?;
    let (player_exp, player_level) = rank_state(db, player_id).await?;
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
        player_level: Some(player_level),
        player_exp: Some(player_exp),
        gain_milestone_level: Some(
            milestone
                .get(&0)
                .map(|(state, _, _)| *state)
                .unwrap_or_default(),
        ),
    })
}

pub async fn get_act128_milestone_bonus(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
) -> Result<Act128MilestoneClaim, AppError> {
    let activity_id = activity_id.ok_or(AppError::InvalidRequest)?;
    ensure_act128_activity(activity_id)?;
    let (_, player_level) = rank_state(db, player_id).await?;
    let states = activity_state::get(
        db,
        player_id,
        activity_id,
        ActivityStateKind::Act128Milestone,
    )
    .await?;
    let claimed_level = states
        .get(&0)
        .map(|(state, _, _)| *state)
        .unwrap_or_default();
    let levels = config::configs::get()
        .activity128_milestone_levels(claimed_level, player_level)
        .ok_or(AppError::InvalidRequest)?;
    let parsed = parse_milestone_rewards(levels.iter().map(|level| level.bonus.as_str()))?;
    let material_changes = parsed.material_changes();

    let mut tx = db.begin().await?;
    let advanced = activity_state::transition_in_transaction(
        &mut tx,
        player_id,
        activity_id,
        claimed_level,
        ActivityStateSet {
            kind: ActivityStateKind::Act128Milestone,
            entry_id: 0,
            state: player_level,
            progress: 0,
            ext: "",
        },
    )
    .await?;
    if !advanced {
        return Err(AppError::InvalidRequest);
    }
    let rewards = reward::RewardManager::new(player_id)
        .apply_in_transaction(&mut tx, db, parsed)
        .await?;
    tx.commit().await?;

    Ok(Act128MilestoneClaim {
        reply: Act128GetMilestoneBonusReply {
            activity_id: Some(activity_id),
            gain_milestone_level: Some(player_level),
        },
        rewards,
        material_changes,
    })
}

fn parse_milestone_rewards<'a>(
    bonuses: impl IntoIterator<Item = &'a str>,
) -> Result<reward::RewardSet, AppError> {
    let mut parsed = reward::RewardSet::default();
    for bonus in bonuses {
        parsed.extend(reward::parse_strict(bonus)?);
    }
    Ok(parsed)
}

fn ensure_act128_activity(activity_id: i32) -> Result<(), AppError> {
    config::configs::get()
        .activity
        .get(activity_id)
        .filter(|activity| activity.type_id == 128)
        .map(|_| ())
        .ok_or(AppError::InvalidRequest)
}

async fn rank_state(db: &SqlitePool, player_id: i64) -> Result<(i32, i32), AppError> {
    let tables = config::configs::get();
    let currency_id = tables
        .activity128_rank_currency_id()
        .ok_or(AppError::InvalidRequest)?;
    let player_exp = currencies::get_currency(db, player_id, currency_id)
        .await?
        .map(|currency| currency.quantity)
        .unwrap_or_default();
    let player_level = tables
        .activity128_rank(player_exp)
        .ok_or(AppError::InvalidRequest)?;
    Ok((player_exp, player_level))
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
    use sqlx::sqlite::SqlitePoolOptions;

    async fn seed_rank_player(
        db: &SqlitePool,
        player_id: i64,
        exp: i32,
        claimed_level: i32,
    ) -> i32 {
        database::run_migrations(db).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (?, 'act128-rank', 0, 0)",
        )
        .bind(player_id)
        .execute(db)
        .await
        .unwrap();
        let tables = config::configs::get();
        let activity_id = tables.latest_open_activity_id(128).unwrap();
        let currency_id = tables.activity128_rank_currency_id().unwrap();
        sqlx::query(
            "INSERT INTO currencies (user_id, currency_id, quantity)
             VALUES (?, ?, ?)",
        )
        .bind(player_id)
        .bind(currency_id)
        .bind(exp)
        .execute(db)
        .await
        .unwrap();
        if claimed_level != 0 {
            activity_state::set(
                db,
                player_id,
                activity_id,
                ActivityStateSet {
                    kind: ActivityStateKind::Act128Milestone,
                    entry_id: 0,
                    state: claimed_level,
                    progress: 0,
                    ext: "",
                },
            )
            .await
            .unwrap();
        }
        activity_id
    }

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

    #[tokio::test]
    async fn rank_info_and_claim_project_persisted_milestone_state() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let player_id = 8;
        let activity_id = seed_rank_player(&db, player_id, 700, 2).await;

        let before = act128_info(&db, player_id, Some(activity_id))
            .await
            .unwrap();
        assert_eq!(before.player_exp, Some(700));
        assert_eq!(before.player_level, Some(7));
        assert_eq!(before.gain_milestone_level, Some(2));

        let claim = get_act128_milestone_bonus(&db, player_id, Some(activity_id))
            .await
            .unwrap();
        assert_eq!(claim.reply.gain_milestone_level, Some(7));
        assert_eq!(claim.material_changes, vec![(1, 120013, 2), (1, 110404, 1)]);

        let after = act128_info(&db, player_id, Some(activity_id))
            .await
            .unwrap();
        assert_eq!(after.gain_milestone_level, Some(7));
        let quantities = sqlx::query_as::<_, (u32, i32)>(
            "SELECT item_id, quantity FROM items
             WHERE user_id = ? AND item_id IN (110404, 120013)
             ORDER BY item_id",
        )
        .bind(player_id)
        .fetch_all(&db)
        .await
        .unwrap();
        assert_eq!(quantities, vec![(110404, 1), (120013, 2)]);

        assert!(
            get_act128_milestone_bonus(&db, player_id, Some(activity_id))
                .await
                .is_err()
        );
        let unchanged: i32 =
            sqlx::query_scalar("SELECT SUM(quantity) FROM items WHERE user_id = ?")
                .bind(player_id)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(unchanged, 3);
    }

    #[tokio::test]
    async fn empty_milestones_still_advance_the_claim_cursor() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let player_id = 9;
        let activity_id = seed_rank_player(&db, player_id, 400, 2).await;

        let claim = get_act128_milestone_bonus(&db, player_id, Some(activity_id))
            .await
            .unwrap();
        assert_eq!(claim.reply.gain_milestone_level, Some(4));
        assert!(claim.material_changes.is_empty());
        assert_eq!(
            act128_info(&db, player_id, Some(activity_id))
                .await
                .unwrap()
                .gain_milestone_level,
            Some(4)
        );
    }

    #[tokio::test]
    async fn invalid_act128_claims_do_not_create_state_or_rewards() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let player_id = 10;
        let _activity_id = seed_rank_player(&db, player_id, 700, 0).await;
        let wrong_type_id = config::configs::get()
            .activity
            .iter()
            .find(|activity| activity.type_id != 128)
            .unwrap()
            .id;

        assert!(
            get_act128_milestone_bonus(&db, player_id, None)
                .await
                .is_err()
        );
        assert!(
            get_act128_milestone_bonus(&db, player_id, Some(wrong_type_id))
                .await
                .is_err()
        );
        assert!(
            get_act128_milestone_bonus(&db, player_id, Some(i32::MAX))
                .await
                .is_err()
        );
        assert!(act128_info(&db, player_id, None).await.is_err());
        assert!(
            act128_info(&db, player_id, Some(wrong_type_id))
                .await
                .is_err()
        );
        assert!(parse_milestone_rewards(["1#broken#1"]).is_err());
        let state_count: i32 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM user_activity_state
             WHERE user_id = ? AND kind = ?",
        )
        .bind(player_id)
        .bind(ActivityStateKind::Act128Milestone.id())
        .fetch_one(&db)
        .await
        .unwrap();
        let item_count: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE user_id = ?")
            .bind(player_id)
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!((state_count, item_count), (0, 0));
    }

    #[tokio::test]
    async fn concurrent_milestone_claims_grant_rewards_once() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let url = format!(
            "sqlite:file:act128-claim-{}?mode=memory&cache=shared",
            ServerTime::now_ms()
        );
        let db = SqlitePoolOptions::new()
            .max_connections(4)
            .connect(&url)
            .await
            .unwrap();
        let player_id = 11;
        let activity_id = seed_rank_player(&db, player_id, 700, 2).await;

        let (left, right) = tokio::join!(
            get_act128_milestone_bonus(&db, player_id, Some(activity_id)),
            get_act128_milestone_bonus(&db, player_id, Some(activity_id))
        );
        assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
        let loser = match (left, right) {
            (Err(error), Ok(_)) | (Ok(_), Err(error)) => error,
            _ => unreachable!("exactly one claim must succeed"),
        };
        assert!(matches!(loser, AppError::InvalidRequest));
        let total: i32 = sqlx::query_scalar("SELECT SUM(quantity) FROM items WHERE user_id = ?")
            .bind(player_id)
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(total, 3);
        let cursor: i32 = sqlx::query_scalar(
            "SELECT state FROM user_activity_state
             WHERE user_id = ? AND activity_id = ? AND kind = ? AND entry_id = 0",
        )
        .bind(player_id)
        .bind(activity_id)
        .bind(ActivityStateKind::Act128Milestone.id())
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(cursor, 7);
    }
}
