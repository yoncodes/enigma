use super::*;
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::test]
async fn profile_edits_follow_lua_limits_and_item_ownership() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (21, 'profile', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    player_infos::create_player_info(&pool, 21, 0)
        .await
        .unwrap();

    let profile = ProfileManager::new(21);
    profile
        .set_signature(&pool, config::configs::get(), "hello".into())
        .await
        .unwrap();
    assert!(
        profile
            .set_signature(&pool, config::configs::get(), "x".repeat(51))
            .await
            .is_err()
    );
    profile
        .set_birthday(&pool, config::configs::get(), "2000-02-29".into())
        .await
        .unwrap();
    assert!(
        profile
            .set_birthday(&pool, config::configs::get(), "2001-01-01".into())
            .await
            .is_err()
    );
    assert!(
        profile
            .set_player_bg(&pool, config::configs::get(), 210001)
            .await
            .is_err()
    );
    items::add_item_quantity(&pool, 21, 210001, 1)
        .await
        .unwrap();
    profile
        .set_player_bg(&pool, config::configs::get(), 210001)
        .await
        .unwrap();
    profile
        .set_character_age(&pool, config::configs::get(), vec![3, 1])
        .await
        .unwrap();
    assert!(
        profile
            .set_character_age(&pool, config::configs::get(), vec![1, 99])
            .await
            .is_err()
    );

    let profile = player_infos::get_player_info(&pool, 21)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(profile.signature, "hello");
    assert_eq!(profile.birthday, "2000-02-29");
    assert_eq!(profile.bg, 210001);
    let wire: sonettobuf::PlayerInfo = player_infos::get_player_info_data(&pool, 21)
        .await
        .unwrap()
        .unwrap()
        .into();
    assert_eq!(wire.character_age, vec![3, 1]);
}

#[tokio::test]
async fn player_info_repairs_chained_episode_unlocks() {
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
         VALUES (22, 'bgm-profile', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    player_infos::create_player_info(&pool, 22, 0)
        .await
        .unwrap();
    database::db::game::dungeons::update_dungeon_progress(&pool, 22, 105, 10505, 1)
        .await
        .unwrap();
    sqlx::query("UPDATE user_open_infos SET is_open = 0 WHERE user_id = 22 AND open_id = 76")
        .execute(&pool)
        .await
        .unwrap();

    let reply = ProfileManager::new(22)
        .get_player_info(&pool)
        .await
        .unwrap();

    assert!(
        reply
            .openinfos
            .iter()
            .any(|info| info.id == 76 && info.is_open)
    );
}
