use super::*;

#[test]
fn updates_hero_3124_talent_extra_str() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    let level_1 = hero_3124_talent_id(2, 1).unwrap();
    let level_2 = hero_3124_talent_id(2, 2).unwrap();
    let extra = update_talent_extra_str("", 2, 1, level_1, true);
    let extra = update_talent_extra_str(&extra, 2, 2, level_2, true);
    assert_eq!(extra, format!("2#{level_1},{level_2}"));

    let extra = update_talent_extra_str(&extra, 2, 2, level_2, false);
    assert_eq!(extra, format!("2#{level_1}"));
}

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
    let manager = HeroManager::new(30);
    let costs = manager.upgrade_materials(&pool, 75, 5).await.unwrap();
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
}

#[tokio::test]
async fn voice_unlock_requires_an_owned_matching_hero() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (14, 'voice', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    UserHeroModel::new(14, pool.clone())
        .create_hero(3003)
        .await
        .unwrap();

    let manager = HeroManager::new(14);
    assert!(manager.unlock_voice(&pool, 3002, 1_300_302).await.is_err());
    manager.unlock_voice(&pool, 3003, 1_300_302).await.unwrap();
    let unlocked: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM hero_voices WHERE voice_id = 1300302")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unlocked, 1);
}

#[tokio::test]
async fn item_unlock_uses_faith_config_and_existing_hero_storage() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (15, 'item-unlock', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    UserHeroModel::new(15, pool.clone())
        .create_hero(3003)
        .await
        .unwrap();
    let manager = HeroManager::new(15);
    assert!(manager.unlock_item(&pool, 3003, 3).await.is_err());
    sqlx::query("UPDATE heroes SET faith = 100000 WHERE user_id = 15 AND hero_id = 3003")
        .execute(&pool)
        .await
        .unwrap();

    let (_, reward) = manager.unlock_item(&pool, 3003, 3).await.unwrap();

    assert_eq!(reward, (2, 40));
    let unlocked: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM hero_item_unlocks WHERE item_id = 3")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unlocked, 1);
}

#[tokio::test]
async fn stale_skill_upgrade_rolls_back_its_cost() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (16, 'skill-race', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let heroes = UserHeroModel::new(16, pool.clone());
    heroes.create_hero(3125).await.unwrap();
    let item_id = duplicate_item_id(3125).unwrap();
    sqlx::query("INSERT INTO items (user_id, item_id, quantity) VALUES (16, ?, 1)")
        .bind(item_id)
        .execute(&pool)
        .await
        .unwrap();

    let mut tx = pool.begin().await.unwrap();
    reward::consume(
        &mut tx,
        16,
        &reward::RewardSet {
            items: vec![(item_id, 1)],
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(
        !heroes
            .upgrade_ex_skill_in_transaction(&mut tx, 3125, 2, 1)
            .await
            .unwrap()
    );
    tx.rollback().await.unwrap();

    let quantity: i32 =
        sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = 16 AND item_id = ?")
            .bind(item_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(quantity, 1);
}

#[tokio::test]
async fn rank_and_insight_skin_commit_together() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let skin = config::configs::get()
        .skin
        .iter()
        .find(|skin| skin.id % 100 == 2 && skin.gain_approach == 1)
        .unwrap();
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (17, 'rank-skin', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let heroes = UserHeroModel::new(17, pool.clone());
    heroes.create_hero(skin.character_id).await.unwrap();
    let rank = config::configs::get()
        .character_rank(skin.character_id, 3)
        .unwrap();
    let required_level = super::progression::required_rank_level(&rank.requirement).unwrap();
    let costs = crate::reward::parse(&rank.consume);
    crate::reward::RewardManager::new(17)
        .apply(&pool, costs.clone())
        .await
        .unwrap();
    heroes
        .set_rank_and_level(skin.character_id, 2, required_level - 1)
        .await
        .unwrap();

    assert!(
        HeroManager::new(17)
            .rank_up(&pool, skin.character_id)
            .await
            .is_err()
    );
    assert_eq!(heroes.get(skin.character_id).await.unwrap().record.rank, 2);
    for (item_id, amount) in &costs.items {
        let quantity: i32 =
            sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = 17 AND item_id = ?")
                .bind(item_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(quantity, *amount);
    }
    for (currency_id, amount) in &costs.currencies {
        let quantity: i32 = sqlx::query_scalar(
            "SELECT quantity FROM currencies WHERE user_id = 17 AND currency_id = ?",
        )
        .bind(currency_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(quantity, *amount);
    }

    heroes
        .set_rank_and_level(skin.character_id, 2, required_level)
        .await
        .unwrap();
    let (_, _, consumed) = HeroManager::new(17)
        .rank_up(&pool, skin.character_id)
        .await
        .unwrap();

    let mut expected_item_ids = costs.items.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    expected_item_ids.sort_unstable();
    assert_eq!(consumed.item_ids, expected_item_ids);
    assert_eq!(
        consumed.currency_ids,
        costs
            .currencies
            .iter()
            .map(|(id, amount)| (*id, -*amount))
            .collect::<Vec<_>>()
    );
    for (item_id, _) in &costs.items {
        let quantity: i32 =
            sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = 17 AND item_id = ?")
                .bind(item_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(quantity, 0);
    }
    for (currency_id, _) in &costs.currencies {
        let quantity: i32 = sqlx::query_scalar(
            "SELECT quantity FROM currencies WHERE user_id = 17 AND currency_id = ?",
        )
        .bind(currency_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(quantity, 0);
    }
    let hero = heroes.get(skin.character_id).await.unwrap();
    assert_eq!(hero.record.rank, 3);
    assert_eq!(hero.record.skin, skin.id);
    assert!(heroes.has_skin(skin.id).await.unwrap());
}

#[tokio::test]
async fn skin_can_be_owned_before_its_hero_but_not_equipped() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let game_data = config::configs::get();
    let skin = game_data
        .skin
        .iter()
        .find(|skin| {
            skin.character_id > 0
                && game_data.character.get(skin.character_id).is_some()
                && game_data
                    .default_character_skin(skin.character_id)
                    .is_some_and(|default| default.id != skin.id)
        })
        .unwrap();
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (20, 'skin-before-hero', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let heroes = UserHeroModel::new(20, pool.clone());
    let applied = crate::reward::RewardManager::new(20)
        .apply(
            &pool,
            crate::reward::RewardSet {
                skins: vec![(skin.id, 1)],
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(applied.skin_gains.len(), 1);
    assert_eq!(applied.skin_gains[0].skin_id, skin.id);
    assert!(applied.skin_gains[0].first_gain);
    assert!(heroes.has_skin(skin.id).await.unwrap());
    assert!(
        HeroManager::new(20)
            .use_skin(&pool, skin.character_id, skin.id)
            .await
            .is_err()
    );

    heroes.create_hero(skin.character_id).await.unwrap();
    let hero = heroes.get(skin.character_id).await.unwrap();
    assert_ne!(hero.record.skin, skin.id);
    assert!(hero.skin_list.iter().any(|owned| owned.skin == skin.id));
    HeroManager::new(20)
        .use_skin(&pool, skin.character_id, skin.id)
        .await
        .unwrap();
    assert_eq!(
        heroes.get(skin.character_id).await.unwrap().record.skin,
        skin.id
    );
}

#[tokio::test]
async fn profile_rejects_foreign_skins_and_equipment() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let foreign_skin = tables
        .skin
        .iter()
        .find(|skin| skin.character_id == 3125)
        .unwrap();
    let normal = tables
        .equip
        .iter()
        .find(|equip| tables.is_normal_equipment(equip))
        .unwrap();
    let experience = tables
        .equip
        .iter()
        .find(|equip| equip.is_exp_equip == 1)
        .unwrap();
    let universal_id = tables.equip_universal_refine_id().unwrap();
    let special_refine = tables
        .equip
        .iter()
        .find(|equip| equip.is_sp_refine != 0 && equip.id != universal_id)
        .unwrap();

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    for (id, name) in [(18_i64, "profile"), (19, "other")] {
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(id)
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
    }
    let heroes = UserHeroModel::new(18, pool.clone());
    heroes.create_hero(3003).await.unwrap();

    let manager = HeroManager::new(18);
    assert!(
        manager
            .use_skin(&pool, 3003, foreign_skin.id)
            .await
            .is_err()
    );

    let foreign_uid = database::db::game::equipment::add_equipment(&pool, 19, normal.id, 1)
        .await
        .unwrap()[0];
    assert!(
        manager
            .default_equip(&pool, 3003, foreign_uid)
            .await
            .is_err()
    );
    let normal_uid = database::db::game::equipment::add_equipment(&pool, 18, normal.id, 1)
        .await
        .unwrap()[0];
    manager
        .default_equip(&pool, 3003, normal_uid)
        .await
        .unwrap();
    for equip_id in [experience.id, special_refine.id, universal_id] {
        let uid = database::db::game::equipment::add_equipment(&pool, 18, equip_id, 1)
            .await
            .unwrap()[0];
        assert!(manager.default_equip(&pool, 3003, uid).await.is_err());
    }
    let stored: i64 = sqlx::query_scalar(
        "SELECT default_equip_uid FROM heroes WHERE user_id = 18 AND hero_id = 3003",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored, normal_uid);
}

#[tokio::test]
async fn specialization_rejects_the_wrong_hero_and_unknown_weapon_group() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (21, 'specialization', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let heroes = UserHeroModel::new(21, pool.clone());
    heroes.create_hero(3003).await.unwrap();
    heroes.create_hero(3123).await.unwrap();

    assert!(
        HeroManager::new(21)
            .choice_weapon(&pool, 3003, 1001, 0)
            .await
            .is_err()
    );
    assert!(
        HeroManager::new(21)
            .choice_weapon(&pool, 3123, 9999, 0)
            .await
            .is_err()
    );
    assert!(
        HeroManager::new(21)
            .reset_talents(&pool, 3003)
            .await
            .is_err()
    );
}

#[test]
fn birthday_reward_uses_the_matching_day_and_next_configured_gift() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let character = config::configs::get()
        .character
        .iter()
        .find(|character| {
            !character.role_birthday.is_empty() && character.birthday_bonus.contains(';')
        })
        .unwrap();
    let (month, day) = character.role_birthday.split_once('/').unwrap();
    let month = month.parse().unwrap();
    let day = day.parse().unwrap();

    let first = super::profile::birthday_reward(character, 0, month, day).unwrap();
    let second = super::profile::birthday_reward(character, 1, month, day).unwrap();
    assert!(!first.is_empty());
    assert!(!second.is_empty());
    assert!(super::profile::birthday_reward(character, 0, month, day % 28 + 1).is_none());
}

#[tokio::test]
async fn birthday_claim_state_does_not_require_hero_ownership() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (23, 'birthday', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let mut tx = pool.begin().await.unwrap();
    assert!(
        database::db::game::sign_in::claim_hero_birthday_in_transaction(
            &mut tx, 23, 3039, 0, 2026,
        )
        .await
        .unwrap()
    );
    tx.commit().await.unwrap();

    assert_eq!(
        database::db::game::sign_in::get_hero_birthday_claim(&pool, 23, 3039)
            .await
            .unwrap(),
        Some((1, 2026))
    );
}
