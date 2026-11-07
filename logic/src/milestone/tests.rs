use super::*;

async fn test_db() -> SqlitePool {
    config::init(&format!(
        "{}/../data/excel2json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let db = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (1, 'milestone-info', 0, 0)",
    )
    .execute(&db)
    .await
    .unwrap();
    db
}

#[tokio::test]
async fn racing_info_returns_the_persisted_claim_cursor() {
    let db = test_db().await;
    let milestone_id = config::get()
        .milestone
        .iter()
        .find(|row| row.r#type == RACING_TYPE)
        .unwrap()
        .milestone_id;
    assert_eq!(
        MilestoneManager::new(1)
            .info(&db, vec![milestone_id])
            .await
            .unwrap()
            .milestone_infos,
        [MilestoneInfo {
            milestone_id: Some(milestone_id),
            get_bonus_id: Some(0),
            progress: Some(0),
        }]
    );
    sqlx::query(
        "INSERT INTO user_milestones (user_id, milestone_id, get_bonus_id)
         VALUES (1, ?, 6)",
    )
    .bind(milestone_id)
    .execute(&db)
    .await
    .unwrap();

    let reply = MilestoneManager::new(1)
        .info(&db, vec![milestone_id])
        .await
        .unwrap();
    assert_eq!(
        reply.milestone_infos,
        [MilestoneInfo {
            milestone_id: Some(milestone_id),
            get_bonus_id: Some(6),
            progress: Some(0),
        }]
    );
    let other_type = config::get()
        .milestone
        .iter()
        .find(|row| row.r#type != RACING_TYPE)
        .unwrap()
        .milestone_id;
    assert!(
        MilestoneManager::new(1)
            .info(&db, vec![other_type])
            .await
            .is_err()
    );
    assert!(MilestoneManager::new(1).info(&db, vec![]).await.is_err());
    assert!(
        MilestoneManager::new(1)
            .info(&db, vec![i32::MAX])
            .await
            .is_err()
    );
}
