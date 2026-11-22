use super::*;
use sqlx::sqlite::SqlitePoolOptions;

pub(super) async fn seed_rank_player(
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
    activity_state::set(
        &db,
        7,
        138520,
        ActivityStateSet {
            kind: ActivityStateKind::Act128ModeTeam,
            entry_id: 2,
            state: 5_000_000,
            progress: 0,
            ext: "",
        },
    )
    .await
    .unwrap();
    team::reset(&db, 7, 138520, 2).await.unwrap();
    let legacy_reset = act128_info(&db, 7, Some(138520)).await.unwrap();
    let legacy_boss = legacy_reset
        .boss_detail
        .iter()
        .find(|boss| boss.boss_id == Some(2))
        .unwrap();
    assert_eq!(legacy_boss.latest_point, Some(0));
    assert_eq!(legacy_reset.max_total_latest_point, Some(5_000_000));

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
    assert_eq!(boss.latest_point, Some(100));
    assert_eq!(boss.act_mode_team, Some(Default::default()));
    assert_eq!(reply.is_activity_mode_open, Some(false));
    assert_eq!(reply.max_total_latest_point, Some(5_000_000));

    team::reset(&db, 7, 138520, 2).await.unwrap();
    let reset = act128_info(&db, 7, Some(138520)).await.unwrap();
    let reset_boss = reset
        .boss_detail
        .iter()
        .find(|boss| boss.boss_id == Some(2))
        .unwrap();
    assert_eq!(reset_boss.latest_point, Some(0));
    assert_eq!(reset.max_total_latest_point, Some(5_000_000));
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
    let unchanged: i32 = sqlx::query_scalar("SELECT SUM(quantity) FROM items WHERE user_id = ?")
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
