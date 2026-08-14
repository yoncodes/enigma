use crate::error::AppError;
use database::db::game::{act233_bp, tasks as task_db};
use sonettobuf::{Act233BpScoreBonusInfo, GetAct233BpInfoReply};
use sqlx::SqlitePool;

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
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_pool() -> SqlitePool {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        database::run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (1, 'act233-info', 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[test]
    fn selects_the_configured_pass_and_exp_per_level() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let pass = config::configs::get()
            .activity233_bp
            .iter()
            .find(|pass| pass.activity_id == 13716)
            .unwrap();

        assert_eq!(pass.bp_id, 1);
        assert_eq!(pass.exp_level_up, 100);
    }

    #[tokio::test]
    async fn get_task_false_omits_tasks_but_keeps_state_projection() {
        let pool = test_pool().await;
        let reply = get_info(&pool, 1, Some(13716), false).await.unwrap();

        assert_eq!(reply.activity_id, Some(13716));
        assert_eq!(reply.bp_id, Some(1));
        assert_eq!(reply.score, Some(0));
        assert_eq!(reply.pay_status, Some(0));
        assert!(reply.task_info.is_empty());
        assert!(reply.score_bonus_info.is_empty());
    }

    #[tokio::test]
    async fn unlocked_bonus_projection_uses_persisted_claim_flags() {
        let pool = test_pool().await;
        get_info(&pool, 1, Some(13716), false).await.unwrap();
        sqlx::query(
            "UPDATE user_act233_bp_state
             SET score = 400, pay_status = 1,
                 has_get_free_bonus = '[2]', has_get_pay_bonus = '[1]'
             WHERE user_id = 1 AND activity_id = 13716 AND bp_id = 1",
        )
        .execute(&pool)
        .await
        .unwrap();

        let reply = get_info(&pool, 1, Some(13716), true).await.unwrap();
        assert_eq!(reply.task_info.len(), 15);
        assert_eq!(reply.score_bonus_info.len(), 4);
        assert_eq!(reply.score_bonus_info[0].level, Some(1));
        assert_eq!(reply.score_bonus_info[0].has_getfree_bonus, Some(false));
        assert_eq!(reply.score_bonus_info[0].has_get_pay_bonus, Some(true));
        assert_eq!(reply.score_bonus_info[1].level, Some(2));
        assert_eq!(reply.score_bonus_info[1].has_getfree_bonus, Some(true));
        assert_eq!(reply.score_bonus_info[1].has_get_pay_bonus, Some(false));
    }

    #[tokio::test]
    async fn unknown_activity_is_invalid_request() {
        let pool = test_pool().await;
        assert!(matches!(
            get_info(&pool, 1, Some(99999), true).await,
            Err(AppError::InvalidRequest)
        ));
    }
}
