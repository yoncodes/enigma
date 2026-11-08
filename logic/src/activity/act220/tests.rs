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
         VALUES (1, 'act220-episode', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}

#[tokio::test]
async fn finish_episode_persists_and_unlocks_configured_successor() {
    let pool = test_pool().await;
    let initial = act220_info(&pool, 1, Some(13904)).await.unwrap();
    assert_eq!(initial.episodes, vec![episode_record(1390401, None)]);

    let episode_ids = [
        1390401, 1390402, 1390403, 1390404, 1390405, 1390406, 1390407, 1390408, 1390409,
    ];
    for (index, episode_id) in episode_ids.iter().copied().enumerate() {
        let finish =
            finish_act220_episode(&pool, 1, Some(13904), Some(episode_id), Some(String::new()))
                .await
                .unwrap();
        assert_eq!(finish.reply.activity_id, Some(13904));
        assert_eq!(finish.reply.episode_id, Some(episode_id));
        assert_eq!(
            finish.episode_push.episodes,
            episode_ids
                .get(index + 1)
                .map(|next| vec![episode_record(*next, None)])
                .unwrap_or_default()
        );
    }

    let persisted = act220_info(&pool, 1, Some(13904)).await.unwrap();
    assert_eq!(persisted.episodes.len(), episode_ids.len());
    assert!(
        persisted
            .episodes
            .iter()
            .all(|episode| episode.is_finished == Some(true))
    );
}

#[tokio::test]
async fn finish_episode_rejects_locked_and_duplicate_progress() {
    let pool = test_pool().await;
    assert!(matches!(
        finish_act220_episode(&pool, 1, Some(13904), Some(1390402), None).await,
        Err(AppError::InvalidRequest)
    ));
    finish_act220_episode(&pool, 1, Some(13904), Some(1390401), None)
        .await
        .unwrap();
    assert!(matches!(
        finish_act220_episode(&pool, 1, Some(13904), Some(1390401), None).await,
        Err(AppError::InvalidRequest)
    ));
}
