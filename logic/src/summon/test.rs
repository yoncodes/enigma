use super::{
    GachaRules, SummonManager,
    commands::{is_newbie_pool, is_newbie_six_star, select_summon_cost, validate_summon_count},
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
async fn summon_info_uses_current_catalog_and_persists_special_pool_type() {
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
    sqlx::query(
        "INSERT INTO user_summon_pools (user_id, pool_id, created_at, updated_at)
         VALUES (29, 385111, 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_sp_pool_info (user_id, pool_id, sp_type)
         VALUES (29, 385111, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let reply = SummonManager::new(29).info(&pool).await.unwrap();
    let current_pool_ids = config::configs::get()
        .current_summon_pools()
        .map(|pool| pool.id)
        .collect::<std::collections::HashSet<_>>();
    assert!(current_pool_ids.contains(&385141));
    assert!(!current_pool_ids.contains(&38151));
    assert!(reply.pool_infos.iter().all(|info| {
        info.pool_id
            .is_some_and(|pool_id| matches!(pool_id, 1 | 2) || current_pool_ids.contains(&pool_id))
    }));
    let info = reply
        .pool_infos
        .iter()
        .find(|info| info.pool_id == Some(385111))
        .unwrap();

    assert_eq!(
        info.sp_pool_info.as_ref().and_then(|info| info.r#type),
        Some(21)
    );
    assert_eq!(
        summon::get_sp_pool_info(&pool, 29, 385111)
            .await
            .unwrap()
            .map(|info| info.sp_type),
        Some(21)
    );
}

#[tokio::test]
async fn newbie_banner_is_lifetime_state_completed_by_its_six_star() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (25, 'newbie-summon', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::starter_data::load_all_starter_data(&pool, 25)
        .await
        .unwrap();

    let pool_config = config::configs::get().summon_pool.get(1).unwrap();
    assert!(is_newbie_pool(pool_config));
    assert!(is_newbie_six_star(1, 3056));
    assert!(!is_newbie_six_star(1, 3005));

    let mut tx = pool.begin().await.unwrap();
    summon::record_summon(&mut tx, 25, 10, true, false)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let active = summon::get_summon_stats(&pool, 25).await.unwrap();
    assert!(active.is_show_new_summon);
    assert_eq!(active.new_summon_count, 10);
    assert_eq!(active.total_summon_count, 10);

    let mut tx = pool.begin().await.unwrap();
    summon::record_summon(&mut tx, 25, 10, true, true)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let completed = summon::get_summon_stats(&pool, 25).await.unwrap();
    assert!(!completed.is_show_new_summon);
    assert_eq!(completed.new_summon_count, 20);
    assert_eq!(completed.total_summon_count, 20);
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
