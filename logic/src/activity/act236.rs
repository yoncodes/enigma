use super::*;
use database::db::game::activity236;
use sonettobuf::{Act236Info, GetAct236InfoReply};

const ACTIVITY_TYPE_ID: i32 = 236;

pub async fn act236_info(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
) -> Result<GetAct236InfoReply, AppError> {
    let activity_id = resolve_activity_id(activity_id)?;
    let state = activity236::get_or_create_state(db, player_id, activity_id).await?;

    Ok(GetAct236InfoReply {
        info: Some(Act236Info {
            activity_id: Some(activity_id),
            score: Some(state.score),
            gain_reward_ids: state.gain_reward_ids,
        }),
    })
}

fn resolve_activity_id(activity_id: Option<i32>) -> Result<i32, AppError> {
    let tables = config::configs::get();
    let activity_id = activity_id.ok_or(AppError::InvalidRequest)?;

    tables
        .activity
        .get(activity_id)
        .filter(|activity| activity.type_id == ACTIVITY_TYPE_ID)
        .map(|_| activity_id)
        .ok_or(AppError::InvalidRequest)
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
             VALUES (1, 'act236-info', 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn info_projects_catalog_default_and_persisted_state() {
        let pool = test_pool().await;
        let activity_id = config::configs::get()
            .latest_open_activity_id(ACTIVITY_TYPE_ID)
            .unwrap();

        let initial = act236_info(&pool, 1, Some(activity_id)).await.unwrap();
        assert_eq!(
            initial.info,
            Some(Act236Info {
                activity_id: Some(activity_id),
                score: Some(0),
                gain_reward_ids: Vec::new(),
            })
        );

        sqlx::query(
            "UPDATE user_activity236_state
             SET score = 240, gain_reward_ids = '[3,7]'
             WHERE user_id = 1 AND activity_id = ?",
        )
        .bind(activity_id)
        .execute(&pool)
        .await
        .unwrap();

        let persisted = act236_info(&pool, 1, Some(activity_id)).await.unwrap();
        assert_eq!(
            persisted.info,
            Some(Act236Info {
                activity_id: Some(activity_id),
                score: Some(240),
                gain_reward_ids: vec![3, 7],
            })
        );
    }

    #[tokio::test]
    async fn info_rejects_activity_outside_type_236() {
        let pool = test_pool().await;
        let activity_id = config::configs::get()
            .activity
            .iter()
            .find(|activity| activity.type_id != ACTIVITY_TYPE_ID)
            .map(|activity| activity.id)
            .unwrap();

        assert!(matches!(
            act236_info(&pool, 1, Some(activity_id)).await,
            Err(AppError::InvalidRequest)
        ));
        assert!(matches!(
            act236_info(&pool, 1, None).await,
            Err(AppError::InvalidRequest)
        ));
        let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_activity236_state")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rows, 0);
    }
}
