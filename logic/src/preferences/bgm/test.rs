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

    let selected_bgm = config::configs::get().bgm_switch.iter().next().unwrap().id;
    bgm::unlock_bgms(&pool, 7, &[selected_bgm], 1)
        .await
        .unwrap();
    bgm::set_active_bgm(&pool, 7, selected_bgm).await.unwrap();

    let preferences = PreferenceManager::new(7);
    let unlocked = preferences.unlock_all_bgms(&pool).await.unwrap();
    let unlocked_again = preferences.unlock_all_bgms(&pool).await.unwrap();
    let (owned, active) = bgm::load_user_bgm(&pool, 7).await.unwrap();

    assert_eq!(unlocked.len() + 1, config::configs::get().bgm_switch.len());
    assert!(unlocked_again.is_empty());
    assert_eq!(owned.len(), config::configs::get().bgm_switch.len());
    assert_eq!(active, Some(selected_bgm));
}
