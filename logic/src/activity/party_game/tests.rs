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
         VALUES (1, 'party-outside', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}

#[tokio::test]
async fn outside_info_projects_default_and_persisted_bonus_count() {
    let pool = test_pool().await;
    let initial = party_outside_info(&pool, 1, Some(13914)).await.unwrap();
    assert_eq!(initial.activity_id, Some(13914));
    assert_eq!(initial.have_extra_bonus_count, Some(0));

    activity_state::set(
        &pool,
        1,
        13914,
        ActivityStateSet {
            kind: ActivityStateKind::PartyGameExtraBonus,
            entry_id: 0,
            state: 0,
            progress: 4,
            ext: "",
        },
    )
    .await
    .unwrap();

    let persisted = party_outside_info(&pool, 1, Some(13914)).await.unwrap();
    assert_eq!(persisted.have_extra_bonus_count, Some(4));
}

#[tokio::test]
async fn outside_info_rejects_non_party_activity() {
    let pool = test_pool().await;
    let wrong_activity = config::configs::get()
        .activity
        .iter()
        .find(|activity| activity.type_id != ACTIVITY_TYPE_ID)
        .unwrap()
        .id;

    assert!(matches!(
        party_outside_info(&pool, 1, Some(wrong_activity)).await,
        Err(AppError::InvalidRequest)
    ));
    assert!(matches!(
        party_outside_info(&pool, 1, None).await,
        Err(AppError::InvalidRequest)
    ));
}
