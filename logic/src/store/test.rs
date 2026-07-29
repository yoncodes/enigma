use super::{
    StoreManager, battle_pass_pay_status, battle_pass_purchase_bonus, charge_goods_attachment,
    charge_goods_diamond_bonus, goods_store_id, is_time_active, parse_time_millis, purchase_cost,
};
use sqlx::SqlitePool;

#[test]
fn parses_store_goods_fields() {
    assert_eq!(goods_store_id("114"), Some(114));
    assert_eq!(goods_store_id(""), None);
    assert_eq!(parse_time_millis("2023-11-09 04:59:59"), 1_699_505_999_000);
    assert_eq!(parse_time_millis(""), 0);
    assert!(is_time_active("", "", 1_783_166_400_000));
    assert!(!is_time_active(
        "",
        "2026-01-01 04:59:59",
        1_783_166_400_000
    ));
}

#[test]
fn expired_store_goods_from_log_are_inactive() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let goods = config::configs::get().store_goods.get(6_142_802).unwrap();

    assert!(!is_time_active(
        &goods.online_time,
        &goods.offline_time,
        1_783_166_400_000
    ));
}

#[test]
fn expired_charge_goods_from_log_are_inactive() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let now = 1_783_166_400_000;
    let active_ids = config::configs::get()
        .store_charge_goods
        .iter()
        .filter(|goods| {
            goods.is_online && is_time_active(&goods.online_time, &goods.offline_time, now)
        })
        .map(|goods| goods.id)
        .collect::<Vec<_>>();

    assert!(!active_ids.contains(&831004));
    assert!(!active_ids.contains(&831006));
    assert!(!active_ids.contains(&831008));
    assert!(!active_ids.contains(&831016));
    assert!(active_ids.contains(&610001));
}

#[tokio::test]
async fn charge_infos_are_synthesized_without_purchase_rows() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect(":memory:").await.unwrap();

    sqlx::query(
        r#"
        CREATE TABLE user_charge_info (
            user_id INTEGER NOT NULL,
            charge_id INTEGER NOT NULL,
            buy_count INTEGER NOT NULL DEFAULT 0,
            first_charge BOOLEAN NOT NULL DEFAULT 1,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (user_id, charge_id)
        )
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let infos = StoreManager::new(1).charge_infos(&pool).await.unwrap();
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_charge_info")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert!(infos.iter().any(|info| info.id == Some(610001)));
    assert!(infos.iter().all(|info| info.id != Some(831004)));
    assert_eq!(rows, 0);
}

#[tokio::test]
async fn configured_container_store_is_returned_without_goods() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect(":memory:").await.unwrap();

    sqlx::query(
        "CREATE TABLE user_store_goods (
            user_id INTEGER NOT NULL,
            goods_id INTEGER NOT NULL,
            buy_count INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (user_id, goods_id)
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    let reply = StoreManager::new(1).infos(&pool, &[410]).await.unwrap();

    assert_eq!(reply.store_infos.len(), 1);
    assert_eq!(reply.store_infos[0].id, 410);
    assert!(reply.store_infos[0].goods_infos.is_empty());
}

#[test]
fn maps_current_bp_charge_goods_to_pay_status() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let bp = database::db::game::tasks::current_battle_pass().unwrap();
    let paid_score = bp.pay_status2_add_level * bp.exp_level_up;

    assert_eq!(
        battle_pass_pay_status(bp.charge_id1),
        Some((bp.bp_id, 1, 0))
    );
    assert_eq!(
        battle_pass_pay_status(bp.charge_id2),
        Some((bp.bp_id, 2, paid_score))
    );
    assert_eq!(
        battle_pass_pay_status(bp.charge_id1to2),
        Some((bp.bp_id, 2, paid_score))
    );
    assert_eq!(battle_pass_pay_status(-1), None);
}

#[test]
fn maps_current_bp_charge_goods_to_purchase_bonus() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let rewards = battle_pass_purchase_bonus(0, 2);

    assert!(!rewards.material_changes().is_empty());
    assert!(battle_pass_purchase_bonus(2, 2).is_empty());
}

#[test]
fn charge_goods_falls_back_to_product_when_item_is_empty() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let goods = config::configs::get()
        .store_charge_goods
        .get(410001)
        .unwrap();

    assert!(goods.item.is_empty());
    assert_eq!(charge_goods_attachment(goods), goods.product);
}

#[test]
fn charge_goods_adds_first_or_extra_diamond_bonus() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let goods = config::configs::get()
        .store_charge_goods
        .get(410002)
        .unwrap();

    assert_eq!(
        charge_goods_diamond_bonus(goods, true).currencies,
        vec![(1, 300)]
    );
    assert_eq!(
        charge_goods_diamond_bonus(goods, false).currencies,
        vec![(1, 30)]
    );
}

#[test]
fn scales_store_goods_product_by_buy_count() {
    let mut rewards = crate::reward::parse("1#170901#1|2#3#5000");
    rewards.scale(2);

    assert_eq!(rewards.items, vec![(170901, 2)]);
    assert_eq!(rewards.currencies, vec![(3, 10000)]);
}

#[test]
fn store_cost_tiers_follow_existing_buy_count() {
    let costs = purchase_cost("2#10#10|2#10#20", 0, 3);
    assert_eq!(costs.currencies, vec![(10, 10), (10, 20), (10, 20)]);

    let later = purchase_cost("2#10#10|2#10#20", 2, 2);
    assert_eq!(later.currencies, vec![(10, 20), (10, 20)]);
}
