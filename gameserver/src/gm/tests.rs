use super::{
    catalog::{
        MaterialKind, build_hero_upgrade_catalog, dungeon_catalog, entries_for_kind,
        is_premium_hero_skin,
    },
    command::{run_command, workflow::unlock_dungeon},
};
use crate::net::app::AppState;
use database::models::game::heros::UserHeroModel;
use std::collections::HashSet;

#[test]
fn dungeon_catalog_lists_story_and_resource_episodes() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let data = dungeon_catalog().data.unwrap();
    let chapters = data["chapters"].as_array().unwrap();
    let episodes = data["episodes"].as_array().unwrap();

    assert!(!chapters.is_empty());
    assert!(!chapters.iter().any(|chapter| chapter["id"] == 201));
    assert!(episodes.iter().any(|episode| episode["id"] == 10102));
    assert!(episodes.iter().any(|episode| episode["id"] == 40101));
}

#[tokio::test]
async fn dungeon_unlock_response_preserves_completed_episode_alias() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (37, 'dungeon-unlock', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::game::player_infos::create_player_info(&pool, 37, 0)
        .await
        .unwrap();
    let state = Box::leak(Box::new(AppState::new(pool, config::configs::get())));

    let response = unlock_dungeon(state, 37, &["unlock", "stage", "40106"])
        .await
        .unwrap();
    let data = response.data.unwrap();

    assert_eq!(
        data["completedEpisodes"],
        data["passedPrerequisiteEpisodes"]
    );
    assert!(
        data["completedEpisodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|episode_id| *episode_id == 40106)
    );
}

#[test]
fn hero_skin_catalog_excludes_basic_and_insight_skins() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let first_skin_id = tables
        .skin
        .iter()
        .find(|skin| is_premium_hero_skin(tables, skin))
        .unwrap()
        .id;
    let owned = HashSet::from([first_skin_id]);

    let all = entries_for_kind(tables, MaterialKind::Skin, None);
    let unowned = entries_for_kind(tables, MaterialKind::Skin, Some(&owned));

    assert!(all.iter().all(|entry| {
        tables
            .skin
            .get(entry.id)
            .is_some_and(|skin| is_premium_hero_skin(tables, skin))
    }));
    assert!(all.iter().any(|entry| entry.id == first_skin_id));
    assert!(!unowned.iter().any(|entry| entry.id == first_skin_id));
    assert!(!is_premium_hero_skin(
        tables,
        tables.skin.get(301201).unwrap()
    ));
    assert!(is_premium_hero_skin(
        tables,
        tables.skin.get(301202).unwrap()
    ));
}

#[tokio::test]
async fn hero_upgrade_catalog_reports_only_owned_current_progression() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (31, 'hero-upgrade-catalog', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let heroes = UserHeroModel::new(31, pool.clone());
    heroes.create_hero(3003).await.unwrap();
    heroes.create_hero(3052).await.unwrap();
    heroes.set_rank_and_level(3003, 3, 75).await.unwrap();
    heroes.set_rank_and_level(3052, 4, 150).await.unwrap();
    sqlx::query("UPDATE heroes SET talent = 5 WHERE user_id = 31 AND hero_id = 3003")
        .execute(&pool)
        .await
        .unwrap();
    let state = AppState::new(pool, config::configs::get());

    let catalog = build_hero_upgrade_catalog(&state, 31).await.unwrap();

    assert_eq!(catalog.heroes.len(), 2);
    let hero = catalog.heroes.iter().find(|hero| hero.id == 3003).unwrap();
    assert_eq!(
        (hero.id, hero.rank, hero.level, hero.resonance),
        (3003, 2, 5, 5)
    );
    assert_eq!(
        hero.max_editable_resonance,
        state
            .tables
            .character_talent
            .iter()
            .filter(|talent| talent.hero_id == 3003 && talent.requirement <= 3)
            .map(|talent| talent.talent_id)
            .max()
            .unwrap()
    );
    assert!(hero.max_editable_resonance <= hero.max_resonance);
    assert_eq!(
        hero.ranks
            .iter()
            .map(|rank| (rank.rank, rank.level_offset, rank.max_level))
            .collect::<Vec<_>>(),
        vec![(0, 0, 30), (1, 30, 40), (2, 70, 50), (3, 120, 60)]
    );
    assert!(hero.destiny_stones.is_empty());
    let destiny_hero = catalog.heroes.iter().find(|hero| hero.id == 3052).unwrap();
    assert_eq!(
        (
            destiny_hero.destiny_rank,
            destiny_hero.destiny_level,
            destiny_hero.max_destiny_rank,
        ),
        (0, 0, 4)
    );
    assert_eq!(destiny_hero.destiny_stones.len(), 1);
    assert_eq!(destiny_hero.destiny_stones[0].id, 305201);
    assert!(!destiny_hero.destiny_stones[0].unlocked);
    assert_eq!(destiny_hero.destiny_stone_options[0].id, 305201);
}

#[tokio::test]
async fn hero_editor_unlocks_and_equips_destiny_stones() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (46, 'destiny-editor', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let heroes = UserHeroModel::new(46, pool.clone());
    heroes.create_hero(3052).await.unwrap();
    heroes.set_rank_and_level(3052, 3, 75).await.unwrap();
    let state = Box::leak(Box::new(AppState::new(
        pool.clone(),
        config::configs::get(),
    )));

    assert!(run_command(state, 46, "hero destiny 3052 1").await.is_err());
    for _ in 0..2 {
        run_command(state, 46, "hero destiny 3052 305201")
            .await
            .unwrap();
    }
    let catalog = build_hero_upgrade_catalog(state, 46).await.unwrap();
    let hero = &catalog.heroes[0];
    assert_eq!(hero.destiny_stone, 305201);
    assert_eq!(hero.destiny_rank, 1);
    assert_eq!(hero.destiny_level, 1);
    assert!(hero.destiny_stones.is_empty());
    assert!(hero.destiny_stone_options[0].unlocked);
    assert!(hero.max_editable_resonance > hero.resonance);
    let command = format!("hero resonance 3052 {}", hero.max_editable_resonance);
    run_command(state, 46, &command).await.unwrap();
    let updated = heroes.get_hero(3052).await.unwrap().record;
    assert_eq!(updated.talent, hero.max_editable_resonance);
    assert_eq!(updated.destiny_stone, 305201);

    run_command(state, 46, "hero destiny 3052 0").await.unwrap();
    let updated = heroes.get_hero(3052).await.unwrap();
    assert_eq!(updated.record.destiny_stone, 0);
    assert_eq!(updated.record.talent, hero.max_editable_resonance);
    assert!(updated.destiny_stone_unlocks.contains(&305201));
}
