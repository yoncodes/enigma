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
async fn failed_template_switch_preserves_the_active_cube_layout() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (46, 'template-switch', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let heroes = UserHeroModel::new(46, pool.clone());
    heroes.create_hero(3003).await.unwrap();
    heroes
        .replace_talent_cubes(3003, 1, vec![(100, 0, 0, 0)])
        .await
        .unwrap();
    sqlx::query(
        "CREATE TRIGGER reject_template_switch
         BEFORE UPDATE OF use_talent_template_id ON heroes
         BEGIN
             SELECT RAISE(ABORT, 'template switch rejected');
         END",
    )
    .execute(&pool)
    .await
    .unwrap();

    assert!(heroes.switch_talent_template(3003, 2).await.is_err());
    let after = heroes.get_hero(3003).await.unwrap();
    assert_eq!(after.record.use_talent_template_id, 1);
    let active_layout: Vec<(i32, i32, i32, i32)> = sqlx::query_as(
        "SELECT cube_id, direction, pos_x, pos_y FROM hero_talent_cubes WHERE hero_uid = ?",
    )
    .bind(after.record.uid)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(active_layout, [(100, 0, 0, 0)]);
}

#[tokio::test]
async fn hero_editor_sets_configured_resonance_and_the_active_layout() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (45, 'resonance-editor', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let heroes = UserHeroModel::new(45, pool.clone());
    heroes.create_hero(3003).await.unwrap();
    HeroManager::new(45)
        .set_level(&pool, 3003, 75)
        .await
        .unwrap();
    let before = heroes.get_hero(3003).await.unwrap();
    heroes.switch_talent_template(3003, 2).await.unwrap();
    let tables = config::configs::get();
    let talent = tables
        .character_talent
        .iter()
        .filter(|talent| talent.hero_id == 3003 && talent.requirement <= before.record.rank)
        .max_by_key(|talent| talent.talent_id)
        .unwrap();
    let target = talent.talent_id;
    let scheme = tables.default_talent_scheme(talent).unwrap();
    let authored = scheme.placements().unwrap();
    let mut expected = authored.clone();
    assert!(!expected.is_empty());
    expected.sort_unstable();

    for _ in 0..2 {
        heroes.replace_talent_cubes(3003, 2, vec![]).await.unwrap();
        let update = HeroManager::new(45)
            .set_resonance(&pool, 3003, target)
            .await
            .unwrap();
        assert_eq!(update.talent, Some(target));
        assert_eq!(update.use_talent_template_id, Some(2));
        let layout = |cubes: Vec<_>| {
            cubes
                .into_iter()
                .map(|cube: sonettobuf::TalentCubeInfo| {
                    (
                        cube.cube_id.unwrap(),
                        cube.direction.unwrap(),
                        cube.pos_x.unwrap(),
                        cube.pos_y.unwrap(),
                    )
                })
                .collect::<Vec<_>>()
        };
        let sorted = |cubes| {
            let mut placed = layout(cubes);
            placed.sort_unstable();
            placed
        };
        assert_eq!(sorted(update.talent_cube_infos), expected);
        let saved = heroes.get_template_info(3003, 2).await.unwrap();
        assert_eq!(sorted(saved.talent_cube_infos), expected);

        heroes.replace_talent_cubes(3003, 2, vec![]).await.unwrap();
        let loaded = heroes
            .load_talent_scheme(3003, scheme.talent_id, scheme.talent_mould, 2)
            .await
            .unwrap();
        assert_eq!(layout(loaded.talent_cube_infos), authored);
        let snapshot = super::super::snapshot(&pool, heroes.get_hero(3003).await.unwrap())
            .await
            .unwrap();
        assert_eq!(sorted(snapshot.talent_cube_infos), expected);
    }
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
