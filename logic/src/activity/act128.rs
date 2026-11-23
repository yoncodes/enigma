use crate::{error::AppError, reward};
use common::time::ServerTime;
use database::db::game::{
    activity_state::{self, ActivityStateKind, ActivityStateSet},
    currencies, tasks,
};
use database::models::game::tasks::UserTask;
use sonettobuf::{
    Act128ActModeHeroSlot, Act128BossDetail, Act128ChangeActModeReply,
    Act128GetMilestoneBonusReply, Act128GetTotalRewardsReply, Get128InfosReply,
    ResetAct128TeamReply, SetAct128TeamReply,
};
use sqlx::{Sqlite, SqlitePool, Transaction};

pub type Act128MilestoneClaim = reward::RewardedReply<Act128GetMilestoneBonusReply>;
pub type Act128TotalRewardsClaim = reward::RewardedReply<Act128GetTotalRewardsReply>;

mod gallery;
mod team;
pub use gallery::{Act128ExpClaim, get_act128_exp, get_gallery_infos};

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
    let mode = activity_state::get(
        db,
        player_id,
        activity_id,
        ActivityStateKind::Act128ModeOpen,
    )
    .await?;
    let teams = activity_state::get(
        db,
        player_id,
        activity_id,
        ActivityStateKind::Act128ModeTeam,
    )
    .await?;
    let max_total = activity_state::get(
        db,
        player_id,
        activity_id,
        ActivityStateKind::Act128MaxTotalLatestPoint,
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

    let mut boss_detail = Vec::with_capacity(boss_ids.len());
    let mut total_latest_point = 0i32;
    for boss_id in boss_ids {
        let (total, highest, ext) = saved.get(&boss_id).cloned().unwrap_or_default();
        let (latest_point, team_ext) = teams
            .get(&boss_id)
            .map(|(latest, _, ext)| (*latest, ext.as_str()))
            .unwrap_or((0, ""));
        total_latest_point = total_latest_point.saturating_add(latest_point);
        boss_detail.push(Act128BossDetail {
            boss_id: Some(boss_id),
            total_point: Some(total),
            has_get_bonus_ids: claimed_reward_ids(&ext)?,
            highest_point: Some(highest),
            double_num: Some(0),
            layer4_total_point: Some(0),
            layer4_highest_point: Some(0),
            sp_highest_point: Some(0),
            sp_item_type_ids: Vec::new(),
            latest_point: Some(latest_point),
            act_mode_team: Some(
                team::from_state(db, player_id, activity_id, boss_id, team_ext).await?,
            ),
        });
    }

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
        is_activity_mode_open: Some(
            mode.get(&0)
                .map(|(state, _, _)| *state != 0)
                .unwrap_or_default(),
        ),
        max_total_latest_point: Some(
            max_total
                .get(&0)
                .map(|(score, _, _)| *score)
                .unwrap_or(total_latest_point),
        ),
    })
}

pub async fn set_act128_team(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
    boss_id: i32,
    front: Vec<Act128ActModeHeroSlot>,
    back: Vec<Act128ActModeHeroSlot>,
    add_bond_group_id: i32,
) -> Result<SetAct128TeamReply, AppError> {
    team::set(
        db,
        player_id,
        activity_id,
        boss_id,
        front,
        back,
        add_bond_group_id,
    )
    .await
}

pub async fn reset_act128_team(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
    boss_id: i32,
) -> Result<ResetAct128TeamReply, AppError> {
    team::reset(db, player_id, activity_id, boss_id).await
}

pub async fn change_act_mode(
    db: &SqlitePool,
    player_id: i64,
    activity_id: i32,
    open: bool,
) -> Result<Act128ChangeActModeReply, AppError> {
    ensure_act128_activity(activity_id)?;
    activity_state::set(
        db,
        player_id,
        activity_id,
        ActivityStateSet {
            kind: ActivityStateKind::Act128ModeOpen,
            entry_id: 0,
            state: i32::from(open),
            progress: 0,
            ext: "",
        },
    )
    .await?;
    Ok(Act128ChangeActModeReply {
        activity_id: Some(activity_id),
        is_activity_mode_open: Some(open),
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
) -> Result<Vec<UserTask>, AppError> {
    let Some(route) = config::configs::get().activity128_battle(episode_id, battle_id) else {
        return Ok(Vec::new());
    };
    if score <= 0 {
        return Ok(Vec::new());
    }

    let (total, highest) = sqlx::query_as::<_, (i32, i32)>(
        "INSERT INTO user_activity_state
            (user_id, activity_id, kind, entry_id, state, progress, ext, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, '', ?)
         ON CONFLICT(user_id, activity_id, kind, entry_id) DO UPDATE SET
            state = MIN(2147483647, user_activity_state.state + excluded.state),
            progress = MAX(user_activity_state.progress, excluded.progress),
            updated_at = excluded.updated_at
         RETURNING state, progress",
    )
    .bind(player_id)
    .bind(route.activity_id)
    .bind(ActivityStateKind::Act128BossScore.id())
    .bind(route.boss_id)
    .bind(score)
    .bind(score)
    .bind(ServerTime::now_ms())
    .fetch_one(&mut **tx)
    .await?;
    preserve_max_total_latest_in_transaction(tx, player_id, route.activity_id).await?;
    sqlx::query(
        "INSERT INTO user_activity_state
            (user_id, activity_id, kind, entry_id, state, progress, ext, updated_at)
         VALUES (?, ?, ?, ?, ?, 0, '', ?)
         ON CONFLICT(user_id, activity_id, kind, entry_id) DO UPDATE SET
            state = excluded.state,
            updated_at = excluded.updated_at",
    )
    .bind(player_id)
    .bind(route.activity_id)
    .bind(ActivityStateKind::Act128ModeTeam.id())
    .bind(route.boss_id)
    .bind(score)
    .bind(ServerTime::now_ms())
    .execute(&mut **tx)
    .await?;
    let total_latest = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(state), 0) FROM user_activity_state
         WHERE user_id = ? AND activity_id = ? AND kind = ?",
    )
    .bind(player_id)
    .bind(route.activity_id)
    .bind(ActivityStateKind::Act128ModeTeam.id())
    .fetch_one(&mut **tx)
    .await?
    .clamp(0, i64::from(i32::MAX)) as i32;
    sqlx::query(
        "INSERT INTO user_activity_state
            (user_id, activity_id, kind, entry_id, state, progress, ext, updated_at)
         VALUES (?, ?, ?, 0, ?, 0, '', ?)
         ON CONFLICT(user_id, activity_id, kind, entry_id) DO UPDATE SET
            state = MAX(user_activity_state.state, excluded.state),
            updated_at = excluded.updated_at",
    )
    .bind(player_id)
    .bind(route.activity_id)
    .bind(ActivityStateKind::Act128MaxTotalLatestPoint.id())
    .bind(total_latest)
    .bind(ServerTime::now_ms())
    .execute(&mut **tx)
    .await?;
    Ok(tasks::act128::sync_score(
        tx,
        player_id,
        route.activity_id,
        route.boss_id,
        total,
        highest,
    )
    .await?)
}

pub(super) async fn preserve_max_total_latest_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    player_id: i64,
    activity_id: i32,
) -> Result<(), AppError> {
    let previous_total_latest = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(state), 0) FROM user_activity_state
         WHERE user_id = ? AND activity_id = ? AND kind = ?",
    )
    .bind(player_id)
    .bind(activity_id)
    .bind(ActivityStateKind::Act128ModeTeam.id())
    .fetch_one(&mut **tx)
    .await?
    .clamp(0, i64::from(i32::MAX)) as i32;
    sqlx::query(
        "INSERT OR IGNORE INTO user_activity_state
            (user_id, activity_id, kind, entry_id, state, progress, ext, updated_at)
         VALUES (?, ?, ?, 0, ?, 0, '', ?)",
    )
    .bind(player_id)
    .bind(activity_id)
    .bind(ActivityStateKind::Act128MaxTotalLatestPoint.id())
    .bind(previous_total_latest)
    .bind(ServerTime::now_ms())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests;
