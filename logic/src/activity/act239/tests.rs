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
             VALUES (1, 'act239', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}

#[test]
fn open_time_uses_the_server_local_offset() {
    assert_eq!(
        parse_open_time_millis("2026-8-2 05:00:00"),
        Some(1_785_664_800_000)
    );
}

#[tokio::test]
async fn info_hides_future_rows_and_projects_claimed_state() {
    let pool = test_pool().await;
    let rows = &config::configs::get().activity239;
    let activity_id = rows.iter().next().unwrap().activity_id;
    let first_open = parse_open_time_millis(&rows.iter().next().unwrap().open_time).unwrap();
    let third_open = parse_open_time_millis(
        &rows
            .iter()
            .find(|row| row.activity_id == activity_id && row.id == 3)
            .unwrap()
            .open_time,
    )
    .unwrap();

    let before = act239_info_at(&pool, 1, Some(activity_id), first_open - 1)
        .await
        .unwrap();
    assert!(before.bonuss.is_empty());

    let open = act239_info_at(&pool, 1, Some(activity_id), first_open)
        .await
        .unwrap();
    assert_eq!(
        open.bonuss,
        vec![
            Act239BonusNo {
                id: Some(1),
                status: Some(1),
            },
            Act239BonusNo {
                id: Some(2),
                status: Some(1),
            },
        ]
    );
    assert!(matches!(
        act239_bonus_at(&pool, 1, Some(activity_id), Some(3), first_open).await,
        Err(AppError::InvalidRequest)
    ));

    let claim = act239_bonus_at(&pool, 1, Some(activity_id), Some(1), first_open)
        .await
        .unwrap();
    assert_eq!(claim.reply.bonuss[0].status, Some(2));
    assert_eq!(claim.reply.bonuss[1].status, Some(1));
    assert_eq!(claim.material_changes, vec![(2, 2, 60)]);
    assert!(matches!(
        act239_bonus_at(&pool, 1, Some(activity_id), Some(1), first_open).await,
        Err(AppError::InvalidRequest)
    ));
    let currency: i32 =
        sqlx::query_scalar("SELECT quantity FROM currencies WHERE user_id = 1 AND currency_id = 2")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(currency, 60);

    assert_eq!(
        act239_info_at(&pool, 1, Some(activity_id), third_open - 1)
            .await
            .unwrap()
            .bonuss
            .len(),
        2
    );
    assert_eq!(
        act239_info_at(&pool, 1, Some(activity_id), third_open)
            .await
            .unwrap()
            .bonuss
            .len(),
        3
    );

    activity_state::set(
        &pool,
        1,
        activity_id,
        ActivityStateSet {
            kind: ActivityStateKind::Act239Bonus,
            entry_id: 2,
            state: 2,
            progress: 0,
            ext: "",
        },
    )
    .await
    .unwrap();
    let red_dot = act239_red_dot_entries_at(&pool, 1, third_open)
        .await
        .unwrap();
    assert!(red_dot.is_empty());
    act239_bonus_at(&pool, 1, Some(activity_id), Some(3), third_open)
        .await
        .unwrap();
    assert!(
        act239_red_dot_entries_at(&pool, 1, third_open)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(super::super::schedule::get(activity_id).is_none());
}
