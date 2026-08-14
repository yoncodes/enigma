use super::*;
use database::db::game::activity236;
use sonettobuf::{Act236GetAutoGainRewardReply, Act236Info, GetAct236InfoReply};
use std::collections::HashSet;

const ACTIVITY_TYPE_ID: i32 = 236;

pub struct Act236RewardClaim {
    pub reply: Act236GetAutoGainRewardReply,
    pub rewards: reward::AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
    pub red_dot_id: i32,
    pub red_dot_value: i32,
}

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

pub async fn act236_get_auto_gain_reward(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
    reward_ids: Vec<i32>,
) -> Result<Act236RewardClaim, AppError> {
    let activity_id = resolve_activity_id(activity_id)?;
    if reward_ids.is_empty()
        || reward_ids.iter().copied().collect::<HashSet<_>>().len() != reward_ids.len()
    {
        return Err(AppError::InvalidRequest);
    }

    let tables = config::configs::get();
    let activity = tables
        .activity
        .get(activity_id)
        .ok_or(AppError::InvalidRequest)?;
    if activity.red_dot_id <= 0 {
        return Err(AppError::InvalidRequest);
    }

    let mut parsed = reward::RewardSet::default();
    for reward_id in &reward_ids {
        let row = tables
            .activity236
            .get(*reward_id)
            .filter(|row| row.activity_id == activity_id)
            .ok_or(AppError::InvalidRequest)?;
        parsed.extend(reward::parse_strict(&row.reward)?);
    }
    let material_changes = parsed.material_changes();

    let state = activity236::get_or_create_state(db, player_id, activity_id).await?;
    for reward_id in &reward_ids {
        let row = tables
            .activity236
            .get(*reward_id)
            .ok_or(AppError::InvalidRequest)?;
        if row.cost > state.score || state.gain_reward_ids.contains(reward_id) {
            return Err(AppError::InvalidRequest);
        }
    }

    let mut gained = state.gain_reward_ids.clone();
    gained.extend(reward_ids.iter().copied());
    let mut tx = db.begin().await?;
    if !activity236::claim_rewards_in_transaction(&mut tx, player_id, activity_id, &state, &gained)
        .await?
    {
        return Err(AppError::InvalidRequest);
    }
    let rewards = reward::apply_in_transaction(&mut tx, db, player_id, parsed).await?;
    tx.commit().await?;

    let red_dot_value = i32::from(tables.activity236.iter().any(|row| {
        row.activity_id == activity_id && row.cost <= state.score && !gained.contains(&row.id)
    }));

    Ok(Act236RewardClaim {
        reply: Act236GetAutoGainRewardReply {
            activity_id: Some(activity_id),
            gain_reward_ids: reward_ids,
        },
        rewards,
        material_changes,
        red_dot_id: activity.red_dot_id,
        red_dot_value,
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

    fn activity_and_rewards() -> (i32, Vec<(i32, i32)>) {
        let tables = config::configs::get();
        let activity_id = tables.latest_open_activity_id(ACTIVITY_TYPE_ID).unwrap();
        let mut rewards = tables
            .activity236
            .iter()
            .filter(|row| row.activity_id == activity_id)
            .map(|row| (row.id, row.cost))
            .collect::<Vec<_>>();
        rewards.sort_unstable_by_key(|(id, _)| *id);
        (activity_id, rewards)
    }

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

    #[tokio::test]
    async fn zero_cost_claim_grants_configured_reward() {
        let pool = test_pool().await;
        let (activity_id, rewards) = activity_and_rewards();
        let zero_cost = rewards
            .iter()
            .find(|(_, cost)| *cost == 0)
            .map(|(id, _)| *id)
            .unwrap();

        let claim = act236_get_auto_gain_reward(&pool, 1, Some(activity_id), vec![zero_cost])
            .await
            .unwrap();
        assert_eq!(claim.reply.activity_id, Some(activity_id));
        assert_eq!(claim.reply.gain_reward_ids, vec![zero_cost]);
        assert_eq!(claim.material_changes, vec![(2, 2, 100)]);
        assert_eq!(
            claim.red_dot_id,
            config::configs::get()
                .activity
                .get(activity_id)
                .unwrap()
                .red_dot_id
        );
        assert_eq!(claim.red_dot_value, 0);

        assert_eq!(
            activity236::get_state(&pool, 1, activity_id)
                .await
                .unwrap()
                .gain_reward_ids,
            vec![zero_cost]
        );
        let currency: i32 = sqlx::query_scalar(
            "SELECT quantity FROM currencies WHERE user_id = 1 AND currency_id = 2",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(currency, 100);
    }

    #[tokio::test]
    async fn bulk_claim_preserves_captured_order_and_existing_claims_without_changing_score() {
        let pool = test_pool().await;
        let (activity_id, configured) = activity_and_rewards();
        let first_id = configured[0].0;
        let reward_ids = configured
            .iter()
            .skip(1)
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        let score = 19_460;
        sqlx::query(
            "INSERT INTO user_activity236_state
             (user_id, activity_id, score, gain_reward_ids)
             VALUES (1, ?, ?, ?)",
        )
        .bind(activity_id)
        .bind(score)
        .bind(serde_json::to_string(&vec![first_id]).unwrap())
        .execute(&pool)
        .await
        .unwrap();

        let claim = act236_get_auto_gain_reward(&pool, 1, Some(activity_id), reward_ids.clone())
            .await
            .unwrap();
        assert_eq!(claim.reply.gain_reward_ids, reward_ids);

        let state = activity236::get_state(&pool, 1, activity_id).await.unwrap();
        let mut expected_union = vec![first_id];
        expected_union.extend(reward_ids.iter().copied());
        assert_eq!(state.gain_reward_ids, expected_union);
        assert_eq!(state.score, score);

        let ordinary_quantity: i32 =
            sqlx::query_scalar("SELECT SUM(quantity) FROM items WHERE user_id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let power_quantity: i32 =
            sqlx::query_scalar("SELECT SUM(quantity) FROM power_items WHERE user_id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!((ordinary_quantity, power_quantity), (14, 5));

        assert!(matches!(
            act236_get_auto_gain_reward(&pool, 1, Some(activity_id), reward_ids).await,
            Err(AppError::InvalidRequest)
        ));
        let quantities: (i32, i32) = sqlx::query_as(
            "SELECT
                (SELECT SUM(quantity) FROM items WHERE user_id = 1),
                (SELECT SUM(quantity) FROM power_items WHERE user_id = 1)",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(quantities, (14, 5));
    }

    #[tokio::test]
    async fn invalid_and_replayed_claims_never_grant_rewards() {
        let pool = test_pool().await;
        let (activity_id, rewards) = activity_and_rewards();
        let zero_cost = rewards.iter().find(|(_, cost)| *cost == 0).unwrap().0;
        let locked = rewards.iter().find(|(_, cost)| *cost > 0).unwrap().0;

        for reward_ids in [
            Vec::new(),
            vec![zero_cost, zero_cost],
            vec![i32::MAX],
            vec![locked],
        ] {
            assert!(matches!(
                act236_get_auto_gain_reward(&pool, 1, Some(activity_id), reward_ids).await,
                Err(AppError::InvalidRequest)
            ));
        }

        act236_get_auto_gain_reward(&pool, 1, Some(activity_id), vec![zero_cost])
            .await
            .unwrap();
        assert!(matches!(
            act236_get_auto_gain_reward(&pool, 1, Some(activity_id), vec![zero_cost]).await,
            Err(AppError::InvalidRequest)
        ));
        let currency: i32 = sqlx::query_scalar(
            "SELECT quantity FROM currencies WHERE user_id = 1 AND currency_id = 2",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(currency, 100);
    }

    #[tokio::test]
    async fn concurrent_claims_grant_the_reward_once() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let url = format!(
            "sqlite:file:act236-claim-{}?mode=memory&cache=shared",
            common::time::ServerTime::now_ms()
        );
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect(&url)
            .await
            .unwrap();
        database::run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (2, 'act236-concurrent', 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let (activity_id, rewards) = activity_and_rewards();
        let zero_cost = rewards.iter().find(|(_, cost)| *cost == 0).unwrap().0;
        activity236::get_or_create_state(&pool, 2, activity_id)
            .await
            .unwrap();

        let (left, right) = tokio::join!(
            act236_get_auto_gain_reward(&pool, 2, Some(activity_id), vec![zero_cost]),
            act236_get_auto_gain_reward(&pool, 2, Some(activity_id), vec![zero_cost])
        );
        assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
        let loser = match (left, right) {
            (Err(error), Ok(_)) | (Ok(_), Err(error)) => error,
            _ => unreachable!("exactly one claim must succeed"),
        };
        assert!(matches!(loser, AppError::InvalidRequest));
        let currency: i32 = sqlx::query_scalar(
            "SELECT quantity FROM currencies WHERE user_id = 2 AND currency_id = 2",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(currency, 100);
    }
}
