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

async fn set_racing_stars(db: &SqlitePool, activity_id: i32, total: i32) {
    sqlx::query("DELETE FROM user_activity243_episodes WHERE user_id = 1 AND activity_id = ?")
        .bind(activity_id)
        .execute(db)
        .await
        .unwrap();
    let mut remaining = total;
    for episode_id in config::get()
        .activity243_episode
        .iter()
        .filter(|episode| episode.activity_id == activity_id)
        .map(|episode| episode.episode_id)
    {
        if remaining == 0 {
            break;
        }
        let stars = remaining.min(3);
        sqlx::query(
            "INSERT INTO user_activity243_episodes
             (user_id, activity_id, episode_id, best_score, best_star, best_time_ms,
              claimed_stars, record, created_at, updated_at)
             VALUES (1, ?, ?, 0, ?, 0, '[]', '', 0, 0)",
        )
        .bind(activity_id)
        .bind(episode_id)
        .bind(stars)
        .execute(db)
        .await
        .unwrap();
        remaining -= stars;
    }
    assert_eq!(remaining, 0);
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

#[tokio::test]
async fn racing_claim_advances_through_captured_thresholds_once() {
    let db = test_db().await;
    let milestone = config::get()
        .milestone
        .iter()
        .find(|row| row.r#type == RACING_TYPE)
        .unwrap();
    let manager = MilestoneManager::new(1);
    for (stars, expected_cursor) in [(3, 2), (6, 3), (14, 6), (18, 9), (20, 11)] {
        set_racing_stars(&db, milestone.activity_id, stars).await;
        let claim = manager.claim(&db, milestone.milestone_id).await.unwrap();
        assert_eq!(claim.reply.milestone_id, Some(milestone.milestone_id));
        assert_eq!(claim.reply.get_bonus_id, Some(expected_cursor));
    }
    assert!(manager.claim(&db, milestone.milestone_id).await.is_err());
    assert_eq!(
        manager
            .info(&db, vec![milestone.milestone_id])
            .await
            .unwrap()
            .milestone_infos[0]
            .get_bonus_id,
        Some(11)
    );
}

#[tokio::test]
async fn stale_claim_cursor_does_not_advance() {
    let db = test_db().await;
    let milestone_id = config::get()
        .milestone
        .iter()
        .find(|row| row.r#type == RACING_TYPE)
        .unwrap()
        .milestone_id;
    sqlx::query(
        "INSERT INTO user_milestones (user_id, milestone_id, get_bonus_id)
         VALUES (1, ?, 3)",
    )
    .bind(milestone_id)
    .execute(&db)
    .await
    .unwrap();

    let mut tx = db.begin().await.unwrap();
    assert!(
        !milestones::advance_claim(&mut tx, 1, milestone_id, 2, 6)
            .await
            .unwrap()
    );
    tx.commit().await.unwrap();
    assert_eq!(
        milestones::claimed_bonus_id(&db, 1, milestone_id)
            .await
            .unwrap(),
        3
    );
}
