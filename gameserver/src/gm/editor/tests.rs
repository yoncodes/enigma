use super::*;

async fn state(player_id: i64) -> &'static AppState {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'muip-editor', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    database::db::game::player_infos::create_player_info(&pool, player_id, 0)
        .await
        .unwrap();
    Box::leak(Box::new(AppState::new(pool, config::configs::get())))
}

#[tokio::test]
async fn bulk_unlocks_follow_config_and_owned_heroes() {
    let state = state(51).await;
    achievements(state, 51, &["unlock", "all"]).await.unwrap();
    backgrounds(state, 51, &["unlock", "all"]).await.unwrap();
    let skin = state
        .tables
        .skin
        .iter()
        .find(|skin| is_premium_hero_skin(state.tables, skin))
        .unwrap();
    let heroes = UserHeroModel::new(51, (*state.db).clone());
    heroes.create_hero(skin.character_id).await.unwrap();
    if skin.character_id != 3012 {
        heroes.create_hero(3012).await.unwrap();
    }
    let equipped_before = heroes
        .get_hero(skin.character_id)
        .await
        .unwrap()
        .record
        .skin;
    let response = garments(state, 51, &["unlock", "all"]).await.unwrap();

    let achievement_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM user_achievements WHERE user_id = 51 AND has_finish = 1",
    )
    .fetch_one(state.db)
    .await
    .unwrap();
    assert_eq!(
        achievement_count,
        state.tables.achievement_task.len() as i64
    );
    for item_id in state
        .tables
        .player_bg
        .iter()
        .map(|background| background.item)
        .filter(|item_id| *item_id > 0)
        .collect::<BTreeSet<_>>()
    {
        let quantity: i32 =
            sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = 51 AND item_id = ?")
                .bind(item_id)
                .fetch_one(state.db)
                .await
                .unwrap();
        assert!(quantity > 0);
    }
    assert!(heroes.has_skin(skin.id).await.unwrap());
    assert!(heroes.has_skin(301202).await.unwrap());
    assert_eq!(
        heroes
            .get_hero(skin.character_id)
            .await
            .unwrap()
            .record
            .skin,
        equipped_before
    );
    assert_eq!(
        response.data.unwrap()["changedSkinIds"],
        serde_json::json!([])
    );
}

#[tokio::test]
async fn psychube_catalog_reports_its_equipped_hero() {
    let state = state(52).await;
    let config = state
        .tables
        .equip
        .iter()
        .find(|equip| state.tables.is_normal_equipment(equip))
        .unwrap();
    let uid = database::db::game::equipment::add_equipment(state.db, 52, config.id, 1)
        .await
        .unwrap()[0];
    let heroes = UserHeroModel::new(52, (*state.db).clone());
    heroes.create_hero(3003).await.unwrap();
    heroes.update_equipped_gear(3003, uid).await.unwrap();

    let catalog = equipment_catalog(state, 52).await.unwrap();
    let entry = catalog
        .equipment
        .iter()
        .find(|entry| entry.uid == uid)
        .unwrap();

    assert_eq!(entry.hero_id, Some(3003));
    assert!(entry.hero_name.is_some());
}

#[test]
fn event_editor_persists_enabled_state_and_millisecond_times() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let path = std::env::temp_dir().join(format!(
        "enigma-event-editor-{}-{}.toml",
        std::process::id(),
        common::time::ServerTime::now_ms()
    ));
    std::fs::copy(
        format!("{}/../common/Config.toml", env!("CARGO_MANIFEST_DIR")),
        &path,
    )
    .unwrap();
    let mut content = std::fs::read_to_string(&path).unwrap();
    content.push_str("\n# operator note\n[operator_extension]\nkeep = true\n");
    std::fs::write(&path, content).unwrap();
    let original = common::config::ServerConfig::load_or_create(&path).unwrap();
    let catalog = build_event_catalog(config::configs::get(), &original.activity.schedule);
    assert_eq!(
        catalog
            .events
            .iter()
            .find(|event| event.id == 13930)
            .unwrap()
            .name,
        "Ducks in the Suitcase"
    );
    assert_eq!(
        catalog
            .events
            .iter()
            .find(|event| event.id == 13919)
            .unwrap()
            .name,
        "Mane's Bulletin Event Mode"
    );
    let event = original.activity.schedule.first().unwrap();
    let id = event.id;
    let start = event.start_time + 1_000;
    let end = event.end_time - 1_000;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    update_event_file(config::configs::get(), &path, id, false, start, end).unwrap();

    let saved = common::config::ServerConfig::load_or_create(&path).unwrap();
    let event = saved
        .activity
        .schedule
        .iter()
        .find(|event| event.id == id)
        .unwrap();
    assert_eq!(
        (event.enabled, event.start_time, event.end_time),
        (false, start, end)
    );
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("# operator note"));
    assert!(content.contains("[operator_extension]"));
    assert!(content.contains("keep = true"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    std::fs::remove_file(path).unwrap();
}
