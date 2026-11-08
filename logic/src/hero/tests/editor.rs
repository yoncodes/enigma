use super::super::*;

#[tokio::test]
async fn portrait_upgrade_uses_owned_shards_before_filling_the_deficit() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (41, 'portrait-editor', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let heroes = UserHeroModel::new(41, pool.clone());
    heroes.create_hero(3125).await.unwrap();
    let item_id = duplicate_item_id(3125).unwrap();
    database::db::game::items::add_item_quantity(&pool, 41, item_id, 2)
        .await
        .unwrap();

    let update = HeroManager::new(41)
        .set_portrait_level(&pool, 3125, 5)
        .await
        .unwrap();

    assert_eq!(update.granted, 3);
    assert_eq!(update.hero.ex_skill_level, Some(5));
    let remaining: i32 =
        sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = 41 AND item_id = ?")
            .bind(item_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn hero_editor_sets_configured_rank_and_level() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (42, 'level-editor', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    UserHeroModel::new(42, pool.clone())
        .create_hero(3003)
        .await
        .unwrap();

    let update = HeroManager::new(42)
        .set_level(&pool, 3003, 75)
        .await
        .unwrap();

    assert_eq!(update.hero.level, Some(75));
    assert_eq!(update.hero.rank, Some(3));
}

#[tokio::test]
async fn bulk_portrait_upgrade_rolls_back_when_any_hero_has_no_config() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (44, 'portrait-rollback', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let heroes = UserHeroModel::new(44, pool.clone());
    heroes.create_hero(3003).await.unwrap();
    heroes.create_hero(3125).await.unwrap();
    sqlx::query("UPDATE heroes SET hero_id = 999999 WHERE user_id = 44 AND hero_id = 3125")
        .execute(&pool)
        .await
        .unwrap();

    assert!(
        HeroManager::new(44)
            .set_all_portrait_levels(&pool, 5)
            .await
            .is_err()
    );

    assert_eq!(
        heroes.get_hero(3003).await.unwrap().record.ex_skill_level,
        0
    );
    let shard_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE user_id = 44")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(shard_rows, 0);
}
