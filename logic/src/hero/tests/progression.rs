use super::*;

#[test]
fn duplicate_item_id_comes_from_character_duplicate_item() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    assert_eq!(duplicate_item_id(3125).unwrap(), 133125);
}

#[tokio::test]
async fn hero_level_up_accepts_levels_between_stat_breakpoints_and_consumes_currency() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (22, 'level-up', 0, 0);
         INSERT INTO currencies (user_id, currency_id, quantity) VALUES
         (22, 3, 1000),
         (22, 5, 1000);",
    )
    .execute(&pool)
    .await
    .unwrap();
    let heroes = UserHeroModel::new(22, pool.clone());
    heroes.create_hero(3023).await.unwrap();

    let (_, updated, consumed) = HeroManager::new(22).level_up(&pool, 3023, 3).await.unwrap();

    assert_eq!(updated.level, Some(3));
    assert_eq!(consumed.currency_ids, vec![(3, -230), (5, -250)]);
    assert!(consumed.material_changes.is_empty());
}

#[tokio::test]
async fn upgrade_materials_fund_the_selected_level_and_resonance() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (30, 'promotion-materials', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let heroes = UserHeroModel::new(30, pool.clone());
    heroes.create_hero(3003).await.unwrap();
    heroes.create_hero(3023).await.unwrap();
    let manager = HeroManager::new(30);
    let costs = manager
        .upgrade_materials(&pool, 3003, 75, 5, None)
        .await
        .unwrap();
    assert!(!costs.items.is_empty());
    assert!(!costs.currencies.is_empty());
    crate::reward::RewardManager::new(30)
        .apply(&pool, costs.clone())
        .await
        .unwrap();

    while heroes.get(3003).await.unwrap().record.level < 75 {
        let current = heroes.get(3003).await.unwrap();
        let level_limit = config::configs::get()
            .character_rank_level_limit(3003, current.record.rank)
            .unwrap();
        if 75 <= level_limit {
            manager.level_up(&pool, 3003, 75).await.unwrap();
            break;
        }
        manager.level_up(&pool, 3003, level_limit).await.unwrap();
        manager.rank_up(&pool, 3003).await.unwrap();
        assert_eq!(
            heroes.get(3003).await.unwrap().record.level,
            level_limit + 1
        );
    }
    let current = heroes.get(3003).await.unwrap();
    assert_eq!((current.record.rank, current.record.level), (3, 75));
    while heroes.get(3003).await.unwrap().record.talent < 5 {
        manager.talent_up(&pool, 3003).await.unwrap();
    }
    assert_eq!(heroes.get(3003).await.unwrap().record.talent, 5);
    assert_eq!(heroes.get(3023).await.unwrap().record.talent, 1);

    for (item_id, _) in costs.items {
        let quantity: i32 =
            sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = 30 AND item_id = ?")
                .bind(item_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(quantity, 0);
    }
    for (currency_id, _) in costs.currencies {
        let quantity: i32 = sqlx::query_scalar(
            "SELECT quantity FROM currencies WHERE user_id = 30 AND currency_id = ?",
        )
        .bind(currency_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(quantity, 0);
    }
}

#[tokio::test]
async fn upgrade_materials_reject_targets_below_owned_progress() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (32, 'promotion-floor', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let heroes = UserHeroModel::new(32, pool.clone());
    heroes.create_hero(3003).await.unwrap();
    heroes.set_rank_and_level(3003, 3, 75).await.unwrap();
    sqlx::query("UPDATE heroes SET talent = 5 WHERE user_id = 32 AND hero_id = 3003")
        .execute(&pool)
        .await
        .unwrap();

    let result = HeroManager::new(32)
        .upgrade_materials(&pool, 3003, 74, 5, None)
        .await;

    assert!(matches!(result, Err(AppError::InvalidRequest)));
}

#[test]
fn destiny_progression_follows_the_configured_slot_order() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    let first = next_destiny_slot(3052, 0, 0).unwrap();
    assert_eq!((first.stage, first.node), (1, 1));

    let last_stage_one = config::configs::get()
        .character_destiny_slots
        .iter()
        .filter(|slot| slot.slots_id == 3052 && slot.stage == 1)
        .map(|slot| slot.node)
        .max()
        .unwrap();
    let next_rank = next_destiny_slot(3052, 1, last_stage_one).unwrap();
    assert_eq!((next_rank.stage, next_rank.node), (2, 1));
    assert_eq!(destiny_stones(3052), vec![305201]);
    assert!(!destiny_available(3052, 4, 149));
    assert!(destiny_available(3052, 4, 150));
    assert!(!destiny_available(3020, 3, 121));
    assert!(destiny_available(3020, 4, 121));
}

#[tokio::test]
async fn destiny_red_dot_reads_set_the_captured_bits() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (34, 'destiny-red-dot', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let heroes = UserHeroModel::new(34, pool.clone());
    heroes.create_hero(3052).await.unwrap();
    sqlx::query("UPDATE heroes SET red_dot = 1 WHERE user_id = 34 AND hero_id = 3052")
        .execute(&pool)
        .await
        .unwrap();
    let manager = HeroManager::new(34);

    let first = manager.read_red_dot(&pool, 3052, 1).await.unwrap();
    assert_eq!(first.red_dot, Some(2));
    sqlx::query("UPDATE heroes SET red_dot = 1 WHERE user_id = 34 AND hero_id = 3052")
        .execute(&pool)
        .await
        .unwrap();
    let second = manager.read_red_dot(&pool, 3052, 2).await.unwrap();
    assert_eq!(second.red_dot, Some(6));
    assert_eq!(heroes.get(3052).await.unwrap().record.red_dot, 6);
    assert!(matches!(
        manager.read_red_dot(&pool, 3052, 3).await,
        Err(AppError::InvalidRequest)
    ));
}

#[tokio::test]
async fn upgrade_materials_include_missing_destiny_slots_and_locked_stone() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (33, 'destiny-materials', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let heroes = UserHeroModel::new(33, pool.clone());
    heroes.create_hero(3052).await.unwrap();
    heroes.set_rank_and_level(3052, 4, 150).await.unwrap();
    sqlx::query(
        "UPDATE heroes SET destiny_rank = 2, destiny_level = 3
         WHERE user_id = 33 AND hero_id = 3052",
    )
    .execute(&pool)
    .await
    .unwrap();

    let costs = HeroManager::new(33)
        .upgrade_materials(
            &pool,
            3052,
            150,
            1,
            Some(DestinyMaterialTarget {
                rank: 4,
                stone_id: 305201,
            }),
        )
        .await
        .unwrap();
    let tables = config::configs::get();
    let slots_id = tables.character_destiny(3052).unwrap().slots_id;
    let mut expected_slots = reward::RewardSet::default();
    for slot in tables
        .character_destiny_slots
        .iter()
        .filter(|slot| slot.slots_id == slots_id)
        .filter(|slot| slot.stage <= 4)
        .filter(|slot| slot.stage > 2 || (slot.stage == 2 && slot.node > 3))
    {
        expected_slots.extend(reward::parse(&slot.consume));
    }
    let mut expected = expected_slots.clone();
    expected.extend(reward::parse(
        &config::configs::get()
            .character_destiny_stone_cost(305201)
            .unwrap()
            .consume,
    ));
    let mut actual_changes = costs.material_changes();
    let mut expected_changes = expected.material_changes();
    actual_changes.sort_unstable();
    expected_changes.sort_unstable();
    assert_eq!(actual_changes, expected_changes);

    let hero_uid: i64 =
        sqlx::query_scalar("SELECT uid FROM heroes WHERE user_id = 33 AND hero_id = 3052")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO hero_destiny_stone_unlocks (hero_uid, stone_id) VALUES (?, ?)")
        .bind(hero_uid)
        .bind(305201)
        .execute(&pool)
        .await
        .unwrap();
    let unlocked_costs = HeroManager::new(33)
        .upgrade_materials(
            &pool,
            3052,
            150,
            1,
            Some(DestinyMaterialTarget {
                rank: 4,
                stone_id: 305201,
            }),
        )
        .await
        .unwrap();
    let mut unlocked_changes = unlocked_costs.material_changes();
    let mut expected_slot_changes = expected_slots.material_changes();
    unlocked_changes.sort_unstable();
    expected_slot_changes.sort_unstable();
    assert_eq!(unlocked_changes, expected_slot_changes);
}
