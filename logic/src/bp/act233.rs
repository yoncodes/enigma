use crate::{error::AppError, reward};
use database::db::game::{act233_bp, tasks as task_db};
use database::models::game::heros::UserHeroModel;
use sonettobuf::{Act233BpScoreBonusInfo, GetAct233BpBonusReply, GetAct233BpInfoReply};
use sqlx::SqlitePool;

pub struct Act233BpBonusClaim {
    pub reply: GetAct233BpBonusReply,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
}

pub async fn claim_bonus(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
    level: Option<i32>,
    pay_bonus: Option<bool>,
) -> Result<Act233BpBonusClaim, AppError> {
    let activity_id = activity_id.ok_or(AppError::InvalidRequest)?;
    let level = level.ok_or(AppError::InvalidRequest)?;
    if level < 0 {
        return Err(AppError::InvalidRequest);
    }
    let pay_bonus = pay_bonus.unwrap_or(false);
    if level == 0 && pay_bonus {
        return Err(AppError::InvalidRequest);
    }

    let tables = config::configs::get();
    let bp = tables
        .activity233_bp
        .iter()
        .find(|bp| bp.activity_id == activity_id)
        .ok_or(AppError::InvalidRequest)?;
    if bp.exp_level_up <= 0 {
        return Err(AppError::InvalidRequest);
    }

    let mut bonuses = tables
        .activity233_lv_bonus
        .iter()
        .filter(|bonus| bonus.bp_id == bp.bp_id)
        .collect::<Vec<_>>();
    if bonuses.is_empty() || bonuses.iter().any(|bonus| bonus.level <= 0) {
        return Err(AppError::InvalidRequest);
    }
    bonuses.sort_unstable_by_key(|bonus| bonus.level);

    let state = act233_bp::get_or_create_state(db, player_id, activity_id, bp.bp_id).await?;
    let unlocked_level = (state.score / bp.exp_level_up).max(0);
    let owned_skins = UserHeroModel::new(player_id, db.clone())
        .get_skins()
        .await?;
    let mut rewards = reward::RewardSet::default();
    let mut free_levels = Vec::new();
    let mut pay_levels = Vec::new();

    if level == 0 {
        for bonus in bonuses
            .into_iter()
            .filter(|bonus| bonus.level <= unlocked_level)
        {
            if state.has_get_free_bonus.contains(&bonus.level) {
                continue;
            }

            let parsed = super::tasks::parse_bp_reward(&bonus.free_bonus, &owned_skins);
            if parsed.is_empty() {
                continue;
            }
            rewards.extend(parsed);
            free_levels.push(bonus.level);
        }

        if free_levels.is_empty() {
            return Err(AppError::InvalidRequest);
        }
    } else {
        let bonus = bonuses
            .into_iter()
            .find(|bonus| bonus.level == level)
            .ok_or(AppError::InvalidRequest)?;
        if level > unlocked_level {
            return Err(AppError::InvalidRequest);
        }

        let already_claimed = if pay_bonus {
            state.has_get_pay_bonus.contains(&level)
        } else {
            state.has_get_free_bonus.contains(&level)
        };
        if already_claimed || (pay_bonus && state.pay_status <= 0) {
            return Err(AppError::InvalidRequest);
        }

        let reward_value = if pay_bonus {
            &bonus.pay_bonus
        } else {
            &bonus.free_bonus
        };
        let parsed = super::tasks::parse_bp_reward(reward_value, &owned_skins);
        if parsed.is_empty() {
            return Err(AppError::InvalidRequest);
        }
        rewards.extend(parsed);
        if pay_bonus {
            pay_levels.push(level);
        } else {
            free_levels.push(level);
        }
    }

    let material_changes = rewards.material_changes();
    let mut tx = db.begin().await?;
    let state = act233_bp::claim_bonus_levels_in_transaction(
        &mut tx,
        player_id,
        activity_id,
        bp.bp_id,
        &state,
        &free_levels,
        &pay_levels,
    )
    .await?
    .ok_or(AppError::InvalidRequest)?;
    let applied_rewards = reward::apply_in_transaction(&mut tx, db, player_id, rewards).await?;
    tx.commit().await?;

    let score_bonus_info = if level > 0 {
        vec![Act233BpScoreBonusInfo {
            level: Some(level),
            has_getfree_bonus: Some(state.has_get_free_bonus.contains(&level)),
            has_get_pay_bonus: Some(state.has_get_pay_bonus.contains(&level)),
        }]
    } else {
        free_levels
            .into_iter()
            .map(|level| Act233BpScoreBonusInfo {
                level: Some(level),
                has_getfree_bonus: Some(true),
                has_get_pay_bonus: None,
            })
            .collect()
    };

    Ok(Act233BpBonusClaim {
        reply: GetAct233BpBonusReply {
            activity_id: Some(activity_id),
            bp_id: Some(bp.bp_id),
            score_bonus_info,
        },
        rewards: applied_rewards,
        material_changes,
    })
}

pub async fn get_info(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
    include_tasks: bool,
) -> Result<GetAct233BpInfoReply, AppError> {
    let activity_id = activity_id.ok_or(AppError::InvalidRequest)?;
    let tables = config::configs::get();
    let bp = tables
        .activity233_bp
        .iter()
        .find(|bp| bp.activity_id == activity_id)
        .ok_or(AppError::InvalidRequest)?;
    if bp.exp_level_up <= 0 {
        return Err(AppError::InvalidRequest);
    }

    let mut bonuses = tables
        .activity233_lv_bonus
        .iter()
        .filter(|bonus| bonus.bp_id == bp.bp_id)
        .collect::<Vec<_>>();
    if bonuses.is_empty() || bonuses.iter().any(|bonus| bonus.level <= 0) {
        return Err(AppError::InvalidRequest);
    }
    bonuses.sort_unstable_by_key(|bonus| bonus.level);

    let configured_task_ids = tables
        .activity233_task
        .iter()
        .filter(|task| {
            task.activity_id == activity_id && task.bp_id == bp.bp_id && task.is_online != 0
        })
        .map(|task| task.id)
        .collect::<std::collections::HashSet<_>>();
    if configured_task_ids.is_empty() {
        return Err(AppError::InvalidRequest);
    }

    task_db::ensure_tasks_for_type(db, player_id, task_db::TaskType::ActBp).await?;
    let state = act233_bp::get_or_create_state(db, player_id, activity_id, bp.bp_id).await?;
    let task_info = if include_tasks {
        task_db::list_act_bp(db, player_id, activity_id)
            .await?
            .into_iter()
            .filter(|task| configured_task_ids.contains(&task.task_id))
            .map(Into::into)
            .collect()
    } else {
        Vec::new()
    };
    let unlocked_level = (state.score / bp.exp_level_up).max(0);
    let score_bonus_info = bonuses
        .into_iter()
        .filter(|bonus| bonus.level <= unlocked_level)
        .map(|bonus| Act233BpScoreBonusInfo {
            level: Some(bonus.level),
            has_getfree_bonus: Some(state.has_get_free_bonus.contains(&bonus.level)),
            has_get_pay_bonus: Some(state.has_get_pay_bonus.contains(&bonus.level)),
        })
        .collect();

    Ok(GetAct233BpInfoReply {
        activity_id: Some(activity_id),
        bp_id: Some(bp.bp_id),
        score: Some(state.score),
        pay_status: Some(state.pay_status),
        task_info,
        score_bonus_info,
    })
}

#[cfg(test)]
mod tests;
