use super::{
    GachaRules, SummonManager,
    commands::{is_newbie_pool, select_summon_cost, validate_summon_count},
    parse_ids, visible_summon_pool_ids_at,
};
use crate::reward::{self, RewardSet};
use database::{
    db::game::{guides, summon},
    models::game::{heros::UserHeroModel, items::UserItemModel},
};
use sqlx::SqlitePool;

#[test]
fn summon_rules_follow_pool_weights_and_pity() {
    let newbie =
        GachaRules::from_values(1, "5#150|4#850|3#4000|2#4500|1#500", "30|30", "5#500").unwrap();
    assert_eq!(newbie.six_rate(29), 0.015);
    assert_eq!(newbie.six_rate(30), 1.0);

    let normal =
        GachaRules::from_values(2, "5#150|4#850|3#4000|2#4500|1#500", "60|70", "5#500").unwrap();
    assert_eq!(normal.six_rate(60), 0.015);
    assert_eq!(normal.six_rate(61), 0.04);
    assert_eq!(normal.six_rate(70), 1.0);

    let lucky =
        GachaRules::from_values(5, "5#150|4#850|3#4000|2#4500|1#500", "30|40", "5#1000").unwrap();
    assert_eq!(lucky.six_rate(31), 0.115);
    assert_eq!(lucky.six_rate(40), 1.0);
}

#[test]
fn summon_count_is_exactly_one_or_ten() {
    assert!(validate_summon_count(1).is_ok());
    assert!(validate_summon_count(10).is_ok());
    for count in [0, 2, 9, 11] {
        assert!(validate_summon_count(count).is_err());
    }
}

#[tokio::test]
async fn teaching_summon_uses_captured_result_and_advances_guide() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (26, 'teaching-summon', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO guide_progress (user_id, guide_id, step_id) VALUES (26, 103, 0)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO items (user_id, item_id, quantity) VALUES (26, 140001, 1)")
        .execute(&pool)
        .await
        .unwrap();

    let completion = SummonManager::new(26)
        .summon(&pool, 2, Some(103), Some(8), 1)
        .await
        .unwrap();

    assert_eq!(completion.reply.summon_result[0].hero_id, Some(3023));
    assert_eq!(completion.guide_info.map(|info| info.step_id), Some(8));
    assert_eq!(
        guides::get_guide_progress(&pool, 26, 103)
            .await
            .unwrap()
            .unwrap()
            .step_id,
        8
    );
    assert_eq!(
        summon::get_gacha_state(&pool, 26, 2).await.unwrap(),
        Some((1, false))
    );
    assert!(
        UserHeroModel::new(26, pool.clone())
            .get_hero(3023)
            .await
            .is_ok()
    );
    assert_eq!(
        UserItemModel::new(26, pool)
            .get_item(140001)
            .await
            .unwrap()
            .unwrap()
            .quantity,
        0
    );
}

#[tokio::test]
async fn ordinary_summon_still_uses_the_pool_without_advancing_a_guide() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (27, 'ordinary-summon', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO items (user_id, item_id, quantity) VALUES (27, 140001, 1)")
        .execute(&pool)
        .await
        .unwrap();

    let completion = SummonManager::new(27)
        .summon(&pool, 2, None, None, 1)
        .await
        .unwrap();
    let hero_id = completion.reply.summon_result[0].hero_id.unwrap();

    assert!(completion.guide_info.is_none());
    assert!(UserHeroModel::new(27, pool).get_hero(hero_id).await.is_ok());
}

#[tokio::test]
async fn missing_summon_tickets_are_paid_from_the_configured_currency() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (28, 'summon-fallback', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO items (user_id, item_id, quantity) VALUES (28, 140001, 4)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO currencies (user_id, currency_id, quantity)
         VALUES (28, 2, 1080)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let selected = select_summon_cost(&pool, 28, "1#140002#1|1#140001#10".into())
        .await
        .unwrap();
    assert_eq!(selected.items, [(140001, 4)]);
    assert_eq!(selected.currencies, [(2, 1080)]);
}

#[tokio::test]
async fn current_catalog_and_special_pool_type_follow_config() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (29, 'special-pool', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let current_pool_ids = visible_summon_pool_ids_at(
        chrono::NaiveDateTime::parse_from_str("2026-07-28 12:00:00", "%Y-%m-%d %H:%M:%S")
            .unwrap()
            .and_utc()
            .timestamp() as i32,
    );
    assert!(current_pool_ids.contains(&385141));
    assert!(!current_pool_ids.contains(&38151));
    assert!(current_pool_ids.contains(&11));
    assert!(current_pool_ids.contains(&2));
    assert!(!current_pool_ids.contains(&1));

    let lower_id_catalog = visible_summon_pool_ids_at(
        chrono::NaiveDateTime::parse_from_str("2025-11-05 12:00:00", "%Y-%m-%d %H:%M:%S")
            .unwrap()
            .and_utc()
            .timestamp() as i32,
    );
    assert!(lower_id_catalog.contains(&30151));
    assert!(!lower_id_catalog.contains(&305161));

    summon::ensure_sp_pool_info(&pool, 29, 385111, 21)
        .await
        .unwrap();
    assert_eq!(
        summon::get_sp_pool_info(&pool, 29, 385111)
            .await
            .unwrap()
            .map(|info| info.sp_type),
        Some(21)
    );
}

#[tokio::test]
async fn replacement_newbie_banner_tracks_its_own_thirty_pull_progress() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    config::init(&data_dir).unwrap();
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (25, 'newbie-summon', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_summon_stats
             (user_id, is_show_new_summon, new_summon_count, total_summon_count)
         VALUES (25, 0, 30, 4804)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let pool_config = config::configs::get().summon_pool.get(11).unwrap();
    assert!(is_newbie_pool(pool_config));
    let mut six_stars = config::configs::get()
        .summon
        .iter()
        .filter(|row| row.id == 11 && row.rare == 5)
        .flat_map(|row| parse_ids(&row.summon_id))
        .collect::<Vec<_>>();
    six_stars.sort_unstable();
    assert_eq!(six_stars, [3007, 3088, 3095]);

    sqlx::query(
        "INSERT INTO user_summon_pools
             (user_id, pool_id, summon_count, created_at, updated_at)
         VALUES (25, 11, 20, 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    summon::save_gacha_state(&pool, 25, 11, 4, false)
        .await
        .unwrap();

    let active = SummonManager::new(25).info(&pool).await.unwrap();
    let active_pool = active
        .pool_infos
        .iter()
        .find(|info| info.pool_id == Some(11))
        .unwrap();
    assert_eq!(active.is_show_new_summon, Some(true));
    assert_eq!(active.new_summon_count, Some(20));
    assert_eq!(active_pool.not_ssr_count, Some(4));

    sqlx::query("INSERT INTO items (user_id, item_id, quantity) VALUES (25, 143801, 10)")
        .execute(&pool)
        .await
        .unwrap();
    let final_pull = SummonManager::new(25)
        .summon(&pool, 11, None, None, 10)
        .await
        .unwrap();
    assert_eq!(final_pull.reply.summon_result.len(), 10);
    summon::save_gacha_state(&pool, 25, 11, 14, false)
        .await
        .unwrap();

    let completed = SummonManager::new(25).info(&pool).await.unwrap();
    let completed_pool = completed
        .pool_infos
        .iter()
        .find(|info| info.pool_id == Some(11))
        .unwrap();
    assert_eq!(completed.is_show_new_summon, Some(false));
    assert_eq!(completed.new_summon_count, Some(30));
    assert_eq!(completed_pool.not_ssr_count, Some(14));
}

#[tokio::test]
async fn recommend_popup_count_is_persisted_per_pool_order() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (22, 'popup', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let manager = SummonManager::new(22);
    let first = manager
        .pop_up_recommend_window(&pool, 34111, 1)
        .await
        .unwrap();
    let second = manager
        .pop_up_recommend_window(&pool, 34111, 1)
        .await
        .unwrap();

    assert_eq!(first.pop_up_count, Some(1));
    assert_eq!(second.pop_up_count, Some(2));
}

#[tokio::test]
async fn summon_progress_claims_each_configured_portrayal_once() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (23, 'progress', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_summon_pools
             (user_id, pool_id, summon_count, created_at, updated_at)
             VALUES (23, 305111, 160, 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let manager = SummonManager::new(23);
    let (first, changed) = manager.progress_rewards(&pool, 305111).await.unwrap();
    let (_, repeated) = manager.progress_rewards(&pool, 305111).await.unwrap();

    assert_eq!(first.has_get_reward_progresses, vec![100, 160]);
    assert_eq!(changed, vec![133123, 133123]);
    assert!(repeated.is_empty());
    let quantity: i32 =
        sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = 23 AND item_id = 133123")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(quantity, 2);
}

#[tokio::test]
async fn stale_gacha_state_rolls_back_cost_and_hero_grant() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (24, 'summon-race', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let heroes = UserHeroModel::new(24, pool.clone());
    heroes.create_hero(3125).await.unwrap();
    sqlx::query("INSERT INTO items (user_id, item_id, quantity) VALUES (24, 100, 1)")
        .execute(&pool)
        .await
        .unwrap();
    summon::save_gacha_state(&pool, 24, 1, 1, false)
        .await
        .unwrap();

    let mut tx = pool.begin().await.unwrap();
    reward::consume(
        &mut tx,
        24,
        &RewardSet {
            items: vec![(100, 1)],
            ..Default::default()
        },
    )
    .await
    .unwrap();
    heroes
        .grant_hero_in_transaction(&mut tx, 3125)
        .await
        .unwrap();
    assert!(
        !summon::save_gacha_state_in_transaction(&mut tx, 24, 1, Some((0, false)), 2, false)
            .await
            .unwrap()
    );
    tx.rollback().await.unwrap();

    let quantity: i32 =
        sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = 24 AND item_id = 100")
            .fetch_one(&pool)
            .await
            .unwrap();
    let duplicate_count: i32 = sqlx::query_scalar(
        "SELECT duplicate_count FROM heroes WHERE user_id = 24 AND hero_id = 3125",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(quantity, 1);
    assert_eq!(duplicate_count, 0);
}
