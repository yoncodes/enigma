use crate::{error::AppError, reward};
use common::time::ServerTime;
use database::db::game::{
    activity_state::{self, ActivityStateKind, ActivityStateSet},
    currencies,
};
use sonettobuf::{
    Act128BossDetail, Act128GalleryDetail, Act128GetExpReply, Act128GetMilestoneBonusReply,
    Act128GetTotalRewardsReply, Get128InfosReply, GetGalleryInfosReply,
};
use sqlx::{Sqlite, SqliteConnection, SqlitePool, Transaction};

pub type Act128MilestoneClaim = reward::RewardedReply<Act128GetMilestoneBonusReply>;
pub type Act128TotalRewardsClaim = reward::RewardedReply<Act128GetTotalRewardsReply>;

pub struct Act128ExpClaim {
    pub reply: Act128GetExpReply,
    pub currency_change: Option<(i32, i32)>,
}

// Gallery progress is shared by seasons. Old versions only stored season/stage
// scores; recover those best scores without modifying the player's old records.
async fn gallery_details(
    conn: &mut SqliteConnection,
    player_id: i64,
) -> Result<Vec<Act128GalleryDetail>, AppError> {
    let config = config::activity128_gallery::data()?;
    let rows = sqlx::query_as::<_, (i32, i32, i32, i32, i32, String)>(
        "SELECT activity_id, kind, entry_id, state, progress, ext FROM user_activity_state
         WHERE user_id = ? AND (kind = ? OR (kind = ? AND activity_id = 0))",
    ).bind(player_id)
        .bind(ActivityStateKind::Act128BossScore.id())
        .bind(ActivityStateKind::Act128Gallery.id())
        .fetch_all(conn).await?;
    let mut details = config.gallery.iter().map(|row| Act128GalleryDetail {
        r#type: Some(row.boss_type), have_fight: Some(false),
        accept_exp_point: Some(0), highest_point: Some(0),
    }).collect::<Vec<_>>();
    for (activity_id, kind, entry_id, state, progress, ext) in rows {
        let boss_type = if kind == ActivityStateKind::Act128Gallery.id() {
            Some(entry_id)
        } else {
            config.boss_type(activity_id, entry_id)
        };
        if let Some(detail) = details.iter_mut().find(|d| d.r#type == boss_type) {
            detail.highest_point = Some(detail.highest_point.unwrap_or(0).max(progress.max(0)));
            if kind == ActivityStateKind::Act128Gallery.id() {
                detail.accept_exp_point = Some(state.max(0));
                detail.have_fight = Some(detail.have_fight.unwrap_or(false) || ext == "1" || progress > 0);
            } else if progress > 0 {
                detail.have_fight = Some(true);
            }
        }
    }
    Ok(details)
}

pub async fn get_gallery_infos(
    db: &SqlitePool, player_id: i64, activity_id: Option<i32>,
) -> Result<GetGalleryInfosReply, AppError> {
    let activity_id = activity_id.ok_or(AppError::InvalidRequest)?;
    ensure_act128_activity(activity_id)?;
    let mut conn = db.acquire().await?;
    Ok(GetGalleryInfosReply {
        activity_id: Some(activity_id),
        gallery_detail: gallery_details(&mut conn, player_id).await?,
    })
}

pub async fn get_act128_exp(
    db: &SqlitePool, player_id: i64, activity_id: Option<i32>, boss_type: Option<i32>,
) -> Result<Act128ExpClaim, AppError> {
    let activity_id = activity_id.ok_or(AppError::InvalidRequest)?;
    let boss_type = boss_type.ok_or(AppError::InvalidRequest)?;
    ensure_act128_activity(activity_id)?;
    let config = config::activity128_gallery::data()?;
    let gallery = config.gallery(boss_type).ok_or(AppError::InvalidRequest)?;
    let tables = config::configs::get();
    let currency_id = tables.activity128_rank_currency_id().ok_or(AppError::InvalidRequest)?;
    let now = ServerTime::now_ms();
    let mut tx = db.begin().await?;
    // Acquire the SQLite write lock before reading the claim cursor, so two
    // simultaneous requests cannot both grant the same threshold.
    sqlx::query("INSERT OR IGNORE INTO user_activity_state
        (user_id, activity_id, kind, entry_id, state, progress, ext, updated_at)
        VALUES (?, 0, ?, ?, 0, 0, '', ?)")
        .bind(player_id).bind(ActivityStateKind::Act128Gallery.id()).bind(boss_type).bind(now)
        .execute(&mut *tx).await?;
    let detail = gallery_details(&mut tx, player_id).await?.into_iter()
        .find(|d| d.r#type == Some(boss_type)).ok_or(AppError::InvalidRequest)?;
    let accepted = detail.accept_exp_point.unwrap_or(0);
    let highest = detail.highest_point.unwrap_or(0);
    let mut gained: i32 = 0;
    for (&point, &exp) in gallery.point.iter().zip(&gallery.point_exp) {
        if point > accepted && point <= highest {
            gained = gained.checked_add(exp).ok_or(AppError::InvalidRequest)?;
        }
    }
    let previous_exp = sqlx::query_scalar::<_, i32>(
        "SELECT quantity FROM currencies WHERE user_id = ? AND currency_id = ?")
        .bind(player_id).bind(currency_id).fetch_optional(&mut *tx).await?.unwrap_or(0);
    let exp = previous_exp.checked_add(gained).filter(|v| *v >= 0)
        .ok_or(AppError::InvalidRequest)?;
    let level = tables.activity128_rank(exp).ok_or(AppError::InvalidRequest)?;
    let new_accepted = if gained > 0 { highest.max(accepted) } else { accepted };
    sqlx::query("UPDATE user_activity_state SET state = ?, progress = MAX(progress, ?),
        ext = CASE WHEN ? THEN '1' ELSE ext END, updated_at = ?
        WHERE user_id = ? AND activity_id = 0 AND kind = ? AND entry_id = ?")
        .bind(new_accepted).bind(highest).bind(detail.have_fight.unwrap_or(false)).bind(now)
        .bind(player_id).bind(ActivityStateKind::Act128Gallery.id()).bind(boss_type)
        .execute(&mut *tx).await?;
    if gained > 0 {
        currencies::add_currency_in_transaction(&mut tx, player_id, currency_id, gained, now).await?;
    }
    tx.commit().await?;
    Ok(Act128ExpClaim {
        reply: Act128GetExpReply { activity_id: Some(activity_id), r#type: Some(boss_type),
            accept_exp_point: Some(new_accepted), player_level: Some(level), player_exp: Some(exp) },
        currency_change: (gained > 0).then_some((currency_id, gained)),
    })
}

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

    let boss_detail = boss_ids
        .into_iter()
        .map(|boss_id| {
            let (total, highest, ext) = saved.get(&boss_id).cloned().unwrap_or_default();
            Ok(Act128BossDetail {
                boss_id: Some(boss_id),
                total_point: Some(total),
                has_get_bonus_ids: claimed_reward_ids(&ext)?,
                highest_point: Some(highest),
                double_num: Some(0),
                layer4_total_point: Some(0),
                layer4_highest_point: Some(0),
                sp_highest_point: Some(0),
                ..Default::default()
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;

    Ok(Get128InfosReply {
        activity_id: Some(activity_id),
        boss_detail,
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

pub async fn get_act128_total_rewards(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
    boss_id: Option<i32>,
) -> Result<Act128TotalRewardsClaim, AppError> {
    let activity_id = activity_id.ok_or(AppError::InvalidRequest)?;
    let boss_id = boss_id.ok_or(AppError::InvalidRequest)?;
    ensure_act128_activity(activity_id)?;
    let tables = config::configs::get();
    if !tables
        .activity128_episode
        .iter()
        .any(|row| row.activity_id == activity_id && row.stage == boss_id)
    {
        return Err(AppError::InvalidRequest);
    }

    let mut tx = db.begin().await?;
    let (total_point, old_ext) = sqlx::query_as::<_, (i32, String)>(
        "SELECT state, ext FROM user_activity_state
         WHERE user_id = ? AND activity_id = ? AND kind = ? AND entry_id = ?",
    )
    .bind(player_id)
    .bind(activity_id)
    .bind(ActivityStateKind::Act128BossScore.id())
    .bind(boss_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::InvalidRequest)?;
    let mut claimed_ids = claimed_reward_ids(&old_ext)?;
    let mut rows = tables
        .activity128_rewards
        .iter()
        .filter(|row| {
            row.activity_id == activity_id
                && row.stage == boss_id
                && row.reward_point_num <= total_point
                && !claimed_ids.contains(&row.id)
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.id);
    if rows.is_empty() {
        return Err(AppError::InvalidRequest);
    }

    let mut parsed = reward::RewardSet::default();
    for row in rows {
        parsed.extend(reward::parse_strict(&row.reward)?);
        claimed_ids.push(row.id);
    }
    claimed_ids.sort_unstable();
    claimed_ids.dedup();
    let new_ext = serde_json::to_string(&claimed_ids)?;
    let updated = sqlx::query(
        "UPDATE user_activity_state SET ext = ?, updated_at = ?
         WHERE user_id = ? AND activity_id = ? AND kind = ? AND entry_id = ?
           AND state = ? AND ext = ?",
    )
    .bind(&new_ext)
    .bind(ServerTime::now_ms())
    .bind(player_id)
    .bind(activity_id)
    .bind(ActivityStateKind::Act128BossScore.id())
    .bind(boss_id)
    .bind(total_point)
    .bind(&old_ext)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(AppError::InvalidRequest);
    }

    let material_changes = parsed.material_changes();
    let rewards = reward::RewardManager::new(player_id)
        .apply_in_transaction(&mut tx, db, parsed)
        .await?;
    tx.commit().await?;

    Ok(Act128TotalRewardsClaim {
        reply: Act128GetTotalRewardsReply {
            activity_id: Some(activity_id),
            boss_id: Some(boss_id),
            has_get_bonus_ids: claimed_ids,
        },
        rewards,
        material_changes,
    })
}

fn claimed_reward_ids(ext: &str) -> Result<Vec<i32>, AppError> {
    if ext.is_empty() {
        return Ok(Vec::new());
    }
    let mut ids = serde_json::from_str::<Vec<i32>>(ext)?;
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
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
    if score < 0 {
        return Ok(());
    }

    let boss_config = config::activity128_gallery::data()?;
    if let Some(boss_type) = boss_config.boss_type(route.activity_id, route.boss_id)
        .filter(|boss_type| boss_config.gallery(*boss_type).is_some()) {
        sqlx::query("INSERT INTO user_activity_state
            (user_id, activity_id, kind, entry_id, state, progress, ext, updated_at)
            VALUES (?, 0, ?, ?, 0, ?, '1', ?)
            ON CONFLICT(user_id, activity_id, kind, entry_id) DO UPDATE SET
              progress = MAX(user_activity_state.progress, excluded.progress),
              ext = '1', updated_at = excluded.updated_at")
            .bind(player_id).bind(ActivityStateKind::Act128Gallery.id()).bind(boss_type)
            .bind(score).bind(ServerTime::now_ms()).execute(&mut **tx).await?;
    }
    if score == 0 {
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

    #[tokio::test]
    async fn gallery_restores_historical_scores_and_claims_each_threshold_once() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let activity_id = seed_rank_player(&db, 80, 0, 0).await;
        // Boss type 106 appeared as stage 2 in both of these seasons.
        for (activity, stage, total, best) in [(13609, 2, 12_000_000, 5_100_000),
                                                (138520, 2, 7_000_000, 4_000_000)] {
            activity_state::set(&db, 80, activity, ActivityStateSet {
                kind: ActivityStateKind::Act128BossScore, entry_id: stage, state: total,
                progress: best, ext: "",
            }).await.unwrap();
        }
        let get = || get_gallery_infos(&db, 80, Some(activity_id));
        let before = get().await.unwrap();
        let item = before.gallery_detail.iter().find(|d| d.r#type == Some(106)).unwrap();
        assert_eq!((item.have_fight, item.highest_point, item.accept_exp_point),
                   (Some(true), Some(5_100_000), Some(0)));
        let untouched = before.gallery_detail.iter().find(|d| d.r#type == Some(101)).unwrap();
        assert_eq!((untouched.have_fight, untouched.highest_point), (Some(false), Some(0)));

        let first = get_act128_exp(&db, 80, Some(activity_id), Some(106)).await.unwrap();
        assert_eq!((first.reply.player_exp, first.reply.accept_exp_point), (Some(400), Some(5_100_000)));
        assert_eq!(first.currency_change, Some((3206, 400)));
        let duplicate = get_act128_exp(&db, 80, Some(activity_id), Some(106)).await.unwrap();
        assert_eq!(duplicate.reply.player_exp, Some(400));
        assert_eq!(duplicate.currency_change, None);

        let mut tx = db.begin().await.unwrap();
        settle_act128_score_in_transaction(&mut tx, 80, 13500420, 118353100, 7_500_000)
            .await.unwrap();
        tx.commit().await.unwrap();
        let next = get_act128_exp(&db, 80, Some(activity_id), Some(106)).await.unwrap();
        assert_eq!((next.reply.player_exp, next.reply.accept_exp_point), (Some(450), Some(7_500_000)));
        assert_eq!(next.currency_change, Some((3206, 50)));
        assert!(get_act128_exp(&db, 80, Some(activity_id), Some(9999)).await.is_err());
        let item = get().await.unwrap().gallery_detail.into_iter().find(|d| d.r#type == Some(106)).unwrap();
        assert_eq!((item.have_fight, item.highest_point, item.accept_exp_point),
                   (Some(true), Some(7_500_000), Some(7_500_000)));
    }

    #[tokio::test]
    async fn gallery_records_zero_score_fight_and_transaction_rollback() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let activity_id = seed_rank_player(&db, 81, 0, 0).await;
        let mut tx = db.begin().await.unwrap();
        settle_act128_score_in_transaction(&mut tx, 81, 13500420, 118353100, 0).await.unwrap();
        tx.rollback().await.unwrap();
        let before = get_gallery_infos(&db, 81, Some(activity_id)).await.unwrap();
        assert_eq!(before.gallery_detail.iter().find(|d| d.r#type == Some(106)).unwrap().have_fight, Some(false));
        let mut tx = db.begin().await.unwrap();
        settle_act128_score_in_transaction(&mut tx, 81, 13500420, 118353100, 0).await.unwrap();
        tx.commit().await.unwrap();
        let after = get_gallery_infos(&db, 81, Some(activity_id)).await.unwrap();
        let detail = after.gallery_detail.iter().find(|d| d.r#type == Some(106)).unwrap();
        assert_eq!((detail.have_fight, detail.highest_point), (Some(true), Some(0)));
        let claim = get_act128_exp(&db, 81, Some(activity_id), Some(106)).await.unwrap();
        assert_eq!(claim.currency_change, None);
    }

    #[tokio::test]
    async fn gallery_concurrent_claims_never_double_pay() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let url = format!("sqlite:file:act128-gallery-{}?mode=memory&cache=shared", ServerTime::now_ms());
        let db = SqlitePoolOptions::new().max_connections(4).connect(&url).await.unwrap();
        let activity_id = seed_rank_player(&db, 82, 0, 0).await;
        activity_state::set(&db, 82, 138520, ActivityStateSet {
            kind: ActivityStateKind::Act128BossScore, entry_id: 2, state: 5_100_000,
            progress: 5_100_000, ext: "",
        }).await.unwrap();
        let (left, right) = tokio::join!(
            get_act128_exp(&db, 82, Some(activity_id), Some(106)),
            get_act128_exp(&db, 82, Some(activity_id), Some(106))
        );
        assert!(left.is_ok() || right.is_ok());
        let exp: i32 = sqlx::query_scalar("SELECT quantity FROM currencies WHERE user_id = ? AND currency_id = ?")
            .bind(82).bind(3206).fetch_one(&db).await.unwrap();
        assert_eq!(exp, 400);
        let retry = get_act128_exp(&db, 82, Some(activity_id), Some(106)).await.unwrap();
        assert_eq!(retry.currency_change, None);
        assert_eq!(retry.reply.player_exp, Some(400));
    }

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
