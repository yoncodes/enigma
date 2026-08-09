use super::{
    auto_use_expired_power_items, buy_power, currency_list, exchange_diamond,
    exchange_same_currency, item_rewards, pop_exchange_same_currency, use_insight_item, use_items,
    use_power_item,
};
use common::time::ServerTime;
use database::models::game::heros::{InsightUpgrade, UserHeroModel};
use sonettobuf::M2qEntry;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

fn init_config() {
    let data = std::env::var_os("ENIGMA_BATTLE_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/excel2json"));
    config::init(data.to_str().unwrap()).unwrap();
}

#[test]
fn selector_preserves_selected_reward_type_by_index() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    let rewards = item_rewards(481020, 1, Some(0)).unwrap();

    assert_eq!(rewards.heroes, vec![(3010, 1)]);
    assert!(rewards.items.is_empty());
}

#[test]
fn selector_preserves_selected_reward_type_by_target_id() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    let rewards = item_rewards(481022, 1, Some(3103)).unwrap();

    assert_eq!(rewards.heroes, vec![(3103, 1)]);
    assert!(rewards.items.is_empty());
}

#[test]
fn raw_hero_selector_uses_target_id_as_hero() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    let rewards = item_rewards(520010, 1, Some(3020)).unwrap();

    assert_eq!(rewards.heroes, vec![(3020, 1)]);
    assert!(rewards.items.is_empty());
}

#[tokio::test]
async fn equipment_level_item_consumes_once_and_maxes_the_configured_target() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (24, 'equip-level-item', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO items (user_id, item_id, quantity) VALUES (24, 3858201, 1)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO equipment
                (uid, user_id, equip_id, level, exp, break_lv, count, is_lock, refine_lv,
                 created_at, updated_at)
             VALUES (99, 24, 1572, 1, 0, 0, 1, 1, 5, 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    use_items(
        &pool,
        24,
        vec![M2qEntry {
            material_id: Some(3858201),
            quantity: Some(1),
            ..Default::default()
        }],
        Some(99),
    )
    .await
    .unwrap();

    let equip: (i32, i32, i32) =
        sqlx::query_as("SELECT level, break_lv, refine_lv FROM equipment WHERE uid = 99")
            .fetch_one(&pool)
            .await
            .unwrap();
    let quantity: i32 =
        sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = 24 AND item_id = 3858201")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(equip, (60, 3, 5));
    assert_eq!(quantity, 0);
}

#[tokio::test]
async fn power_item_use_updates_the_owned_stack_and_stamina_together() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (21, 'power', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO power_items (uid, user_id, item_id, quantity, expire_time, created_at)
             VALUES (31, 21, 20, 2, 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let (_, updates) = use_power_item(&pool, 21, 31).await.unwrap();

    assert_eq!(updates[0].quantity, Some(1));
    let stamina: i32 = sqlx::query_scalar(
        "SELECT quantity FROM currencies WHERE user_id = 21 AND currency_id = 4",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stamina, 60);
}

#[tokio::test]
async fn starter_power_matches_level_cap_and_recovers_while_offline() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (31, 'power-recovery', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::starter_data::load_all_starter_data(&pool, 31)
        .await
        .unwrap();

    let initial = currency_list(
        &pool,
        31,
        vec![database::db::game::currencies::POWER_CURRENCY_ID],
    )
    .await
    .unwrap();
    assert_eq!(initial.currency_list[0].quantity, Some(170));

    let power = config::configs::get()
        .currency
        .get(database::db::game::currencies::POWER_CURRENCY_ID)
        .unwrap();
    let interval = i64::from(power.recover_time) * 1_000;
    let last_recover_time = ServerTime::now_ms() - interval * 3;
    sqlx::query(
        "UPDATE currencies
         SET quantity = 100, last_recover_time = ?
         WHERE user_id = 31 AND currency_id = ?",
    )
    .bind(last_recover_time)
    .bind(database::db::game::currencies::POWER_CURRENCY_ID)
    .execute(&pool)
    .await
    .unwrap();

    let recovered = currency_list(
        &pool,
        31,
        vec![database::db::game::currencies::POWER_CURRENCY_ID],
    )
    .await
    .unwrap();
    assert_eq!(recovered.currency_list[0].quantity, Some(103));
    assert_eq!(
        recovered.currency_list[0].last_recover_time,
        Some((last_recover_time + interval * 3) as u64)
    );

    let spend_time = ServerTime::now_ms();
    sqlx::query(
        "UPDATE currencies
         SET quantity = 0, last_recover_time = ?
         WHERE user_id = 31 AND currency_id = ?",
    )
    .bind(spend_time - interval * 3)
    .bind(database::db::game::currencies::POWER_CURRENCY_ID)
    .execute(&pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(
        database::db::game::currencies::consume_currency_in_transaction(
            &mut tx,
            31,
            database::db::game::currencies::POWER_CURRENCY_ID,
            2,
            spend_time,
        )
        .await
        .unwrap()
    );
    tx.commit().await.unwrap();
    let remaining: i32 = sqlx::query_scalar(
        "SELECT quantity FROM currencies WHERE user_id = 31 AND currency_id = ?",
    )
    .bind(database::db::game::currencies::POWER_CURRENCY_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, 1);
}

#[tokio::test]
async fn diamond_exchange_moves_the_requested_amount_atomically() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (22, 'diamond', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO currencies (user_id, currency_id, quantity, last_recover_time, expired_time)
             VALUES (22, 1, 100, 0, 0), (22, 2, 10, 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    exchange_diamond(&pool, 22, 30, 1).await.unwrap();

    let quantities: Vec<(i32, i32)> = sqlx::query_as(
        "SELECT currency_id, quantity FROM currencies WHERE user_id = 22 ORDER BY currency_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(quantities, vec![(1, 70), (2, 40)]);
}

#[tokio::test]
async fn power_purchase_uses_the_next_configured_price() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (23, 'buy-power', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO player_state (player_id, created_at, updated_at) VALUES (23, 0, 0)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO currencies (user_id, currency_id, quantity, last_recover_time, expired_time)
             VALUES (23, 2, 200, ?, 0), (23, 4, 0, ?, 0)",
    )
    .bind(ServerTime::now_ms())
    .bind(ServerTime::now_ms())
    .execute(&pool)
    .await
    .unwrap();

    let (first, _) = buy_power(&pool, 23).await.unwrap();
    let (second, _) = buy_power(&pool, 23).await.unwrap();

    assert_eq!(first.can_buy_count, Some(7));
    assert_eq!(second.can_buy_count, Some(6));
    let quantities: Vec<(i32, i32)> = sqlx::query_as(
        "SELECT currency_id, quantity FROM currencies WHERE user_id = 23 ORDER BY currency_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(quantities, vec![(2, 50), (4, 200)]);
}

#[tokio::test]
async fn exchange_popup_state_stays_on_the_currency_row() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (24, 'exchange', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO currencies (user_id, currency_id, quantity, last_recover_time, expired_time)
             VALUES (24, 28, 0, 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    pop_exchange_same_currency(&pool, 24, vec![28])
        .await
        .unwrap();
    let reply = exchange_same_currency(&pool, 24).await.unwrap();

    assert_eq!(
        reply
            .currency_exchanges
            .iter()
            .find(|row| row.currency_id == Some(28))
            .and_then(|row| row.is_poped),
        Some(1)
    );
}

#[tokio::test]
async fn expired_power_conversion_rolls_back_when_the_set_changed() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (25, 'expired-power-race', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO power_items (uid, user_id, item_id, quantity, expire_time, created_at)
         VALUES (41, 25, 20, 1, 1, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    assert!(
        !database::db::game::items::convert_expired_power_items(&pool, 25, &[41, 42], 4, 60)
            .await
            .unwrap()
    );

    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM power_items WHERE user_id = 25")
        .fetch_one(&pool)
        .await
        .unwrap();
    let stamina: Option<i32> = sqlx::query_scalar(
        "SELECT quantity FROM currencies WHERE user_id = 25 AND currency_id = 4",
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, 1);
    assert_eq!(stamina, None);
}

#[tokio::test]
async fn expired_power_conversion_stops_at_the_configured_stamina_limit() {
    init_config();
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (27, 'expired-power-limit', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO currencies
             (user_id, currency_id, quantity, last_recover_time, expired_time)
         VALUES (27, 4, 1580, ?, 0)",
    )
    .bind(ServerTime::now_ms())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO power_items (uid, user_id, item_id, quantity, expire_time, created_at)
         VALUES (43, 27, 20, 1, 1, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let reply = auto_use_expired_power_items(&pool, 27).await.unwrap();

    assert_eq!(reply.used, Some(true));
    let stamina: i32 = sqlx::query_scalar(
        "SELECT quantity FROM currencies WHERE user_id = 27 AND currency_id = 4",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM power_items WHERE user_id = 27")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stamina, 1600);
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn insight_item_rolls_back_when_hero_progress_changed() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (26, 'insight-race', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let heroes = UserHeroModel::new(26, pool.clone());
    heroes.create_hero(3003).await.unwrap();
    sqlx::query(
        "INSERT INTO insight_items (uid, user_id, item_id, quantity, expire_time)
         VALUES (51, 26, 1, 1, 9999999999)",
    )
    .execute(&pool)
    .await
    .unwrap();

    assert!(
        !heroes
            .apply_insight_item(InsightUpgrade {
                item_uid: 51,
                item_id: 1,
                hero_id: 3003,
                current_rank: 2,
                current_level: 1,
                target_rank: 2,
                target_level: 1,
            })
            .await
            .unwrap()
    );

    let quantity: i32 = sqlx::query_scalar("SELECT quantity FROM insight_items WHERE uid = 51")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(quantity, 1);
}

#[tokio::test]
async fn multi_item_use_rolls_back_when_any_cost_is_missing() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (27, 'multi-item-race', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO items (user_id, item_id, quantity) VALUES (27, 481020, 1)")
        .execute(&pool)
        .await
        .unwrap();

    assert!(
        use_items(
            &pool,
            27,
            vec![
                M2qEntry {
                    material_id: Some(481020),
                    quantity: Some(1),
                    ..Default::default()
                },
                M2qEntry {
                    material_id: Some(481022),
                    quantity: Some(1),
                    ..Default::default()
                },
            ],
            None,
        )
        .await
        .is_err()
    );

    let quantity: i32 =
        sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = 27 AND item_id = 481020")
            .fetch_one(&pool)
            .await
            .unwrap();
    let heroes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM heroes WHERE user_id = 27")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(quantity, 1);
    assert_eq!(heroes, 0);
}

#[tokio::test]
async fn insight_item_rejects_expired_empty_and_wrong_rarity_items() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (28, 'insight-validation', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    UserHeroModel::new(28, pool.clone())
        .create_hero(3003)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO insight_items (uid, user_id, item_id, quantity, expire_time)
         VALUES
             (61, 28, 489094, 1, 1),
             (62, 28, 489094, 0, 9999999999),
             (63, 28, 489093, 1, 9999999999)",
    )
    .execute(&pool)
    .await
    .unwrap();

    for uid in [61, 62, 63] {
        assert!(use_insight_item(&pool, 28, uid, 3003).await.is_err());
    }
    let quantities: Vec<i32> =
        sqlx::query_scalar("SELECT quantity FROM insight_items WHERE user_id = 28 ORDER BY uid")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(quantities, vec![1, 0, 1]);
}
