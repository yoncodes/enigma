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

async fn set_claim_state(pool: &SqlitePool, score: i32, pay_status: i32, free: &str, pay: &str) {
    get_info(pool, 1, Some(13716), false).await.unwrap();
    sqlx::query(
        "UPDATE user_act233_bp_state
             SET score = ?, pay_status = ?,
                 has_get_free_bonus = ?, has_get_pay_bonus = ?
             WHERE user_id = 1 AND activity_id = 13716 AND bp_id = 1",
    )
    .bind(score)
    .bind(pay_status)
    .bind(free)
    .bind(pay)
    .execute(pool)
    .await
    .unwrap();
}

async fn assert_materials_persisted(pool: &SqlitePool, expected: &[(u32, u32, i32)]) {
    for (kind, id, quantity) in expected {
        let stored: i32 = match kind {
            1 => sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = 1 AND item_id = ?")
                .bind(id)
                .fetch_one(pool)
                .await
                .unwrap(),
            2 => sqlx::query_scalar(
                "SELECT quantity FROM currencies WHERE user_id = 1 AND currency_id = ?",
            )
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap(),
            other => panic!("unexpected Act233 test reward material type {other}"),
        };
        assert_eq!(stored, *quantity);
    }
}

#[tokio::test]
async fn single_free_claim_is_config_driven_and_cannot_be_repeated() {
    let pool = test_pool().await;
    set_claim_state(&pool, 100, 0, "[]", "[]").await;
    let bonus = config::configs::get()
        .activity233_lv_bonus
        .iter()
        .find(|bonus| bonus.bp_id == 1 && bonus.level == 1)
        .unwrap();
    let expected = reward::parse(&bonus.free_bonus).material_changes();

    let claim = claim_bonus(&pool, 1, Some(13716), Some(1), Some(false))
        .await
        .unwrap();
    assert_eq!(claim.material_changes, expected);
    assert_eq!(claim.reply.score_bonus_info.len(), 1);
    assert_eq!(claim.reply.score_bonus_info[0].level, Some(1));
    assert_eq!(
        claim.reply.score_bonus_info[0].has_getfree_bonus,
        Some(true)
    );
    assert_eq!(
        claim.reply.score_bonus_info[0].has_get_pay_bonus,
        Some(false)
    );

    let item_change = expected
        .iter()
        .find(|(kind, _, _)| *kind == 1)
        .copied()
        .unwrap();
    let quantity: i32 =
        sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = 1 AND item_id = ?")
            .bind(item_change.1)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(quantity, item_change.2);

    assert!(matches!(
        claim_bonus(&pool, 1, Some(13716), Some(1), Some(false)).await,
        Err(AppError::InvalidRequest)
    ));
    let quantity_after: i32 =
        sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = 1 AND item_id = ?")
            .bind(item_change.1)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(quantity_after, quantity);
}

#[tokio::test]
async fn paid_claim_requires_paid_status_and_updates_only_paid_track() {
    let pool = test_pool().await;
    set_claim_state(&pool, 100, 0, "[]", "[]").await;
    assert!(matches!(
        claim_bonus(&pool, 1, Some(13716), Some(1), Some(true)).await,
        Err(AppError::InvalidRequest)
    ));

    set_claim_state(&pool, 100, 1, "[]", "[]").await;
    let bonus = config::configs::get()
        .activity233_lv_bonus
        .iter()
        .find(|bonus| bonus.bp_id == 1 && bonus.level == 1)
        .unwrap();
    let expected = reward::parse(&bonus.pay_bonus).material_changes();
    let claim = claim_bonus(&pool, 1, Some(13716), Some(1), Some(true))
        .await
        .unwrap();
    assert_eq!(claim.material_changes, expected);
    assert_materials_persisted(&pool, &expected).await;
    assert_eq!(
        claim.reply.score_bonus_info[0].has_getfree_bonus,
        Some(false)
    );
    assert_eq!(
        claim.reply.score_bonus_info[0].has_get_pay_bonus,
        Some(true)
    );
    let state = act233_bp::get_state(&pool, 1, 13716, 1).await.unwrap();
    assert!(state.has_get_free_bonus.is_empty());
    assert_eq!(state.has_get_pay_bonus, vec![1]);
}

#[tokio::test]
async fn zero_level_claims_all_unclaimed_unlocked_free_rows() {
    let pool = test_pool().await;
    set_claim_state(&pool, 400, 1, "[2]", "[]").await;

    let claim = claim_bonus(&pool, 1, Some(13716), Some(0), Some(false))
        .await
        .unwrap();
    let mut expected_rewards = reward::RewardSet::default();
    for bonus in config::configs::get()
        .activity233_lv_bonus
        .iter()
        .filter(|bonus| bonus.bp_id == 1 && [1, 3, 4].contains(&bonus.level))
    {
        expected_rewards.extend(reward::parse(&bonus.free_bonus));
    }
    let expected = expected_rewards.material_changes();
    assert_eq!(claim.material_changes, expected);
    assert_materials_persisted(&pool, &expected).await;
    let levels = claim
        .reply
        .score_bonus_info
        .iter()
        .map(|info| info.level.unwrap())
        .collect::<Vec<_>>();
    assert_eq!(levels, vec![1, 3, 4]);
    assert!(
        claim
            .reply
            .score_bonus_info
            .iter()
            .all(|info| info.has_getfree_bonus == Some(true) && info.has_get_pay_bonus.is_none())
    );
    let state = act233_bp::get_state(&pool, 1, 13716, 1).await.unwrap();
    assert_eq!(state.has_get_free_bonus, vec![2, 1, 3, 4]);
    assert!(state.has_get_pay_bonus.is_empty());
}

#[tokio::test]
async fn invalid_claim_selectors_and_locked_levels_are_rejected() {
    let pool = test_pool().await;
    set_claim_state(&pool, 100, 1, "[]", "[]").await;

    for result in [
        claim_bonus(&pool, 1, Some(13716), Some(-1), Some(false)).await,
        claim_bonus(&pool, 1, Some(13716), Some(0), Some(true)).await,
        claim_bonus(&pool, 1, Some(13716), Some(2), Some(false)).await,
    ] {
        assert!(matches!(result, Err(AppError::InvalidRequest)));
    }
}
