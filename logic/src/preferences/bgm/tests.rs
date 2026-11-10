use super::*;

#[tokio::test]
async fn unlock_all_bgms_inserts_only_missing_tracks_and_preserves_selection() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (7, 'bgm', 0, 0)")
        .execute(&pool)
        .await
        .unwrap();

    let selected_bgm = config::configs::get()
        .bgm_switch
        .iter()
        .find(|bgm| has_global_audio_bank(bgm.id))
        .unwrap()
        .id;
    bgm::unlock_bgms(&pool, 7, &[selected_bgm], 1)
        .await
        .unwrap();
    bgm::set_active_bgm(&pool, 7, selected_bgm).await.unwrap();

    let preferences = PreferenceManager::new(7);
    let unlocked = preferences.unlock_all_bgms(&pool).await.unwrap();
    let unlocked_again = preferences.unlock_all_bgms(&pool).await.unwrap();
    let (owned, active) = bgm::load_user_bgm(&pool, 7).await.unwrap();
    let available_count = config::configs::get()
        .bgm_switch
        .iter()
        .filter(|bgm| has_global_audio_bank(bgm.id))
        .count();

    assert_eq!(unlocked.len() + 1, available_count);
    assert!(unlocked_again.is_empty());
    assert_eq!(owned.len(), available_count);
    assert_eq!(active, Some(selected_bgm));
}

#[tokio::test]
async fn bgm_info_hides_tracks_without_global_audio_banks() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (8, 'bgm', 0, 0)")
        .execute(&pool)
        .await
        .unwrap();

    let unavailable = config::configs::get()
        .bgm_switch
        .iter()
        .find(|bgm| !has_global_audio_bank(bgm.id))
        .unwrap()
        .id;
    let available = config::configs::get()
        .bgm_switch
        .iter()
        .find(|bgm| has_global_audio_bank(bgm.id))
        .unwrap()
        .id;
    bgm::unlock_bgms(&pool, 8, &[available, unavailable], 1)
        .await
        .unwrap();
    bgm::set_active_bgm(&pool, 8, available).await.unwrap();

    let error = PreferenceManager::new(8)
        .set_use_bgm(&pool, unavailable)
        .await
        .unwrap_err();
    assert!(matches!(error, AppError::InvalidRequest));
    assert_eq!(
        bgm::load_user_bgm(&pool, 8).await.unwrap().1,
        Some(available)
    );

    bgm::set_active_bgm(&pool, 8, unavailable).await.unwrap();

    let reply = PreferenceManager::new(8).bgm_info(&pool).await.unwrap();
    let (stored, active) = bgm::load_user_bgm(&pool, 8).await.unwrap();

    assert_eq!(reply.bgm_infos.len(), 1);
    assert!(
        reply
            .bgm_infos
            .iter()
            .all(|info| info.bgm_id.is_some_and(has_global_audio_bank))
    );
    assert_eq!(reply.use_bgm_id, None);
    assert_eq!(stored.len(), 2);
    assert_eq!(active, Some(unavailable));
}
