use super::{
    RewardMaterialType, RewardSet, apply, consume, hero_duplicate_rewards, parse,
    parse_bonus_with_cost,
};

#[test]
fn reward_groups_resolve_cost_based_player_exp_and_currency() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    let rewards = parse_bonus_with_cost(2111004, 25);

    assert_eq!(rewards.player_exp, 250);
    assert!(rewards.currencies.contains(&(3, 250)));
    assert!(rewards.material_changes().contains(&(3, 0, 250)));
}

#[test]
fn parses_bp_score_rewards() {
    let rewards = parse("25#14#10000");

    assert_eq!(rewards.bp_scores, vec![(14, 10000)]);
    assert_eq!(
        rewards.material_changes(),
        vec![(RewardMaterialType::Bp.id(), 14, 10000)]
    );
}

#[test]
fn power_item_id_11_is_not_a_room_building() {
    let rewards = parse("10#11#1");

    assert_eq!(rewards.power_items, vec![(11, 1)]);
    assert!(rewards.room_buildings.is_empty());
    assert_eq!(
        rewards.material_changes(),
        vec![(RewardMaterialType::PowerPotion.id(), 11, 1)]
    );
}

#[test]
fn room_building_rewards_keep_material_type_11() {
    let rewards = parse("11#11311#1");

    assert_eq!(rewards.room_buildings, vec![(11311, 1)]);
    assert!(rewards.items.is_empty());
    assert_eq!(
        rewards.material_changes(),
        vec![(RewardMaterialType::Building.id(), 11311, 1)]
    );
}

#[test]
fn block_package_keeps_material_type_13() {
    let rewards = parse("13#11#1");

    assert_eq!(rewards.block_packages, vec![(11, 1)]);
    assert!(rewards.currencies.is_empty());
    assert_eq!(
        rewards.material_changes(),
        vec![(RewardMaterialType::BlockPackage.id(), 11, 1)]
    );
}

#[test]
fn antique_rewards_keep_material_type_18() {
    let rewards = parse("18#161001#1");

    assert_eq!(rewards.antiques, vec![(161001, 1)]);
    assert_eq!(
        rewards.material_changes(),
        vec![(RewardMaterialType::Antique.id(), 161001, 1)]
    );
}

#[test]
fn parses_player_cloth_rewards() {
    let rewards = parse("7#6#1|8#6#25");

    assert_eq!(rewards.player_cloths, vec![(6, 1)]);
    assert_eq!(rewards.player_cloth_exp, vec![(6, 25)]);
}

#[tokio::test]
async fn currency_reward_creates_missing_balance() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(
        "CREATE TABLE currencies (
                user_id INTEGER NOT NULL,
                currency_id INTEGER NOT NULL,
                quantity INTEGER NOT NULL,
                last_recover_time INTEGER,
                expired_time INTEGER,
                PRIMARY KEY (user_id, currency_id)
            )",
    )
    .execute(&pool)
    .await
    .unwrap();

    apply(&pool, 7, parse("2#11#25")).await.unwrap();
    let quantity: i32 = sqlx::query_scalar(
        "SELECT quantity FROM currencies WHERE user_id = 7 AND currency_id = 11",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(quantity, 25);
}

#[tokio::test]
async fn cost_validation_does_not_partially_consume() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(
        "CREATE TABLE items (
                user_id INTEGER NOT NULL,
                item_id INTEGER NOT NULL,
                quantity INTEGER NOT NULL,
                last_use_time INTEGER,
                last_update_time INTEGER,
                total_gain_count INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (user_id, item_id)
            );
            CREATE TABLE currencies (
                user_id INTEGER NOT NULL,
                currency_id INTEGER NOT NULL,
                quantity INTEGER NOT NULL,
                last_recover_time INTEGER,
                expired_time INTEGER,
                PRIMARY KEY (user_id, currency_id)
            );
            CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                level INTEGER NOT NULL
            );",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO items (user_id, item_id, quantity) VALUES (7, 10, 1)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users (id, level) VALUES (7, 1)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO currencies
         (user_id, currency_id, quantity, last_recover_time)
         VALUES (7, 4, 0, ?)",
    )
    .bind(common::time::ServerTime::now_ms())
    .execute(&pool)
    .await
    .unwrap();

    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(
        consume(&mut tx, 7, &parse("1#10#1|2#4#1")).await,
        Err(crate::error::AppError::InsufficientCurrency)
    ));
    tx.rollback().await.unwrap();
    let quantity: i32 =
        sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = 7 AND item_id = 10")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(quantity, 1);
}

#[tokio::test]
async fn consumed_currency_is_not_reported_as_a_material_delta() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(
        "CREATE TABLE currencies (
            user_id INTEGER NOT NULL,
            currency_id INTEGER NOT NULL,
            quantity INTEGER NOT NULL,
            last_recover_time INTEGER,
            expired_time INTEGER,
            PRIMARY KEY (user_id, currency_id)
        );
        INSERT INTO currencies (user_id, currency_id, quantity) VALUES (7, 3, 100);",
    )
    .execute(&pool)
    .await
    .unwrap();

    let mut tx = pool.begin().await.unwrap();
    let consumed = consume(&mut tx, 7, &parse("2#3#10")).await.unwrap();

    assert_eq!(consumed.currency_ids, vec![(3, -10)]);
    assert!(consumed.material_changes.is_empty());
}

#[test]
fn hero_duplicate_rewards_come_from_character_config() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    let rewards = hero_duplicate_rewards(3125, 1).unwrap();

    assert_eq!(rewards.items, vec![(133125, 1)]);
}

#[tokio::test]
async fn reward_application_rejects_negative_grants() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (8, 'negative-reward', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    assert!(
        apply(
            &pool,
            8,
            RewardSet {
                items: vec![(10, -1)],
                ..Default::default()
            },
        )
        .await
        .is_err()
    );
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE user_id = 8")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0);
}

#[tokio::test]
async fn equipment_rewards_use_unique_uids_across_accounts() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (10, 'first-equip', 0, 0), (11, 'second-equip', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let first = apply(&pool, 10, parse("9#1000#1")).await.unwrap();
    let second = apply(&pool, 11, parse("9#1000#1")).await.unwrap();

    assert_eq!(first.equip_uids, [30_000_000]);
    assert_eq!(second.equip_uids, [30_000_001]);
}
