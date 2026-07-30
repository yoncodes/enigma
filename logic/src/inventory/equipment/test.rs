use super::{
    decompose_config, decompose_count, incremental_exp, is_strengthen_fodder, refine,
    strengthened_level, valid_strengthen_consumes,
};
use database::models::game::equipment::Equipment;
use sonettobuf::EatEquip;
use sqlx::SqlitePool;

#[test]
fn decompose_reward_uses_configured_rarity_exp() {
    assert_eq!(decompose_count("2#200|3#300", [2, 3], 1), Some(5));
    assert_eq!(decompose_config("9#999#1"), Some((999, 1)));
}

#[test]
fn strengthen_rejects_empty_duplicate_and_non_positive_fodder() {
    assert!(!valid_strengthen_consumes(&[]));
    assert!(!valid_strengthen_consumes(&[(1, 1), (1, 1)]));
    assert!(!valid_strengthen_consumes(&[(1, 0)]));
    assert!(valid_strengthen_consumes(&[(1, 1), (2, 2)]));
}

#[test]
fn strengthen_uses_the_configured_equipment_categories() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let universal_id = tables.equip_universal_refine_id().unwrap();
    let normal = tables
        .equip
        .iter()
        .find(|equip| {
            equip.is_exp_equip == 0 && equip.is_sp_refine == 0 && equip.id != universal_id
        })
        .unwrap();
    let experience = tables
        .equip
        .iter()
        .find(|equip| equip.is_exp_equip == 1)
        .unwrap();
    let special = tables
        .equip
        .iter()
        .find(|equip| equip.is_sp_refine != 0)
        .unwrap();
    let universal = tables.equip.get(universal_id).unwrap();

    assert!(tables.is_normal_equipment(normal));
    assert!(!tables.is_normal_equipment(experience));
    assert!(!tables.is_normal_equipment(special));
    assert!(!tables.is_normal_equipment(universal));
    assert!(is_strengthen_fodder(normal, universal_id));
    assert!(is_strengthen_fodder(experience, universal_id));
    assert!(!is_strengthen_fodder(special, universal_id));
    assert!(!is_strengthen_fodder(universal, universal_id));
}

#[test]
fn strengthen_uses_configured_base_exp_level_cost_and_score_cost() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let equip = tables
        .equip
        .iter()
        .find(|equip| equip.is_exp_equip == 0)
        .unwrap();
    let source = Equipment {
        uid: 1,
        user_id: 1,
        equip_id: equip.id,
        level: 1,
        exp: 0,
        break_lv: 0,
        count: 1,
        is_lock: false,
        refine_lv: 1,
        created_at: 0,
        updated_at: 0,
    };
    let base_exp =
        super::config_pair(&tables.equip_const.get(2).unwrap().value, equip.rare).unwrap();
    assert_eq!(incremental_exp(tables, &source, equip), Some(base_exp));

    let next = tables.equip_strengthen_cost(equip.rare, 2).unwrap();
    assert_eq!(
        strengthened_level(tables, equip.rare, 20, 1, 0, next.exp),
        Some((
            2,
            0,
            i32::try_from(i64::from(next.exp) * i64::from(next.score_cost) / 1000).unwrap()
        ))
    );
}

#[tokio::test]
async fn refine_rejects_the_whole_request_when_any_fodder_is_invalid() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let target = config::configs::get()
        .equip
        .iter()
        .find(|equip| {
            equip.is_exp_equip == 0
                && equip.is_sp_refine == 0
                && equip.rare
                    > config::configs::get()
                        .equip_refine_rarity_threshold()
                        .unwrap()
        })
        .unwrap();
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (20, 'refine', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    for (uid, locked) in [(1_i64, false), (2, false), (3, true)] {
        sqlx::query(
            "INSERT INTO equipment
             (uid, user_id, equip_id, level, exp, break_lv, count, is_lock, refine_lv, created_at, updated_at)
             VALUES (?, 20, ?, 1, 0, 0, 1, ?, 1, 0, 0)",
        )
        .bind(uid)
        .bind(target.id)
        .bind(locked)
        .execute(&pool)
        .await
        .unwrap();
    }

    assert!(refine(&pool, 20, 1, vec![2, 3]).await.is_err());
    let target_level: i32 = sqlx::query_scalar("SELECT refine_lv FROM equipment WHERE uid = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let fodder_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM equipment WHERE uid IN (2, 3)")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(target_level, 1);
    assert_eq!(fodder_count, 2);
}

#[tokio::test]
async fn refine_rejects_fodder_after_reaching_the_configured_max() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let target = tables
        .equip
        .iter()
        .find(|equip| {
            equip.is_exp_equip == 0
                && equip.is_sp_refine == 0
                && equip.rare > tables.equip_refine_rarity_threshold().unwrap()
        })
        .unwrap();
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (24, 'refine-cap', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    for (uid, refine_level) in [(20_i64, 1), (21, 4), (22, 1)] {
        sqlx::query(
            "INSERT INTO equipment
             (uid, user_id, equip_id, level, exp, break_lv, count, is_lock, refine_lv,
              created_at, updated_at)
             VALUES (?, 24, ?, 1, 0, 0, 1, 0, ?, 0, 0)",
        )
        .bind(uid)
        .bind(target.id)
        .bind(refine_level)
        .execute(&pool)
        .await
        .unwrap();
    }

    assert!(refine(&pool, 24, 20, vec![21, 22]).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT refine_lv FROM equipment WHERE uid = 20")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM equipment WHERE uid IN (21, 22)")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
}

#[tokio::test]
async fn refine_rejects_non_normal_and_low_rarity_targets() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let material = tables
        .equip
        .iter()
        .find(|equip| equip.is_exp_equip == 1)
        .unwrap();
    let low_rarity = tables
        .equip
        .iter()
        .find(|equip| {
            equip.is_exp_equip == 0
                && equip.is_sp_refine == 0
                && equip.rare <= tables.equip_refine_rarity_threshold().unwrap()
        })
        .unwrap();
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (25, 'refine-targets', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    for (uid, equip_id) in [
        (30_i64, material.id),
        (31, material.id),
        (32, low_rarity.id),
        (33, low_rarity.id),
    ] {
        sqlx::query(
            "INSERT INTO equipment
             (uid, user_id, equip_id, level, exp, break_lv, count, is_lock, refine_lv,
              created_at, updated_at)
             VALUES (?, 25, ?, 1, 0, 0, 1, 0, 1, 0, 0)",
        )
        .bind(uid)
        .bind(equip_id)
        .execute(&pool)
        .await
        .unwrap();
    }

    assert!(refine(&pool, 25, 30, vec![31]).await.is_err());
    assert!(refine(&pool, 25, 32, vec![33]).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM equipment WHERE user_id = 25 AND uid IN (30, 31, 32, 33)"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        4
    );
}

#[tokio::test]
async fn refine_uses_fodder_levels_and_target_config() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let target = tables
        .equip
        .iter()
        .find(|equip| !equip.use_sp_refine.is_empty())
        .unwrap();
    let special_id = target
        .use_sp_refine
        .split('#')
        .next()
        .unwrap()
        .parse::<i32>()
        .unwrap();
    let universal_id = tables.equip_universal_refine_id().unwrap();
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (21, 'refine-levels', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    for (uid, equip_id, refine_level) in [
        (10_i64, target.id, 1),
        (11, target.id, 2),
        (12, special_id, 1),
        (13, universal_id, 1),
    ] {
        sqlx::query(
            "INSERT INTO equipment
             (uid, user_id, equip_id, level, exp, break_lv, count, is_lock, refine_lv,
              created_at, updated_at)
             VALUES (?, 21, ?, 1, 0, 0, 1, 0, ?, 0, 0)",
        )
        .bind(uid)
        .bind(equip_id)
        .bind(refine_level)
        .execute(&pool)
        .await
        .unwrap();
    }

    refine(&pool, 21, 10, vec![11, 12, 13]).await.unwrap();

    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT refine_lv FROM equipment WHERE uid = 10")
            .fetch_one(&pool)
            .await
            .unwrap(),
        tables.equip_max_refine_level().unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM equipment WHERE uid IN (11, 12, 13)")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn strengthen_commits_cost_and_consumed_equipment_together() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let target_config = tables
        .equip
        .iter()
        .find(|equip| {
            tables.is_normal_equipment(equip)
                && tables
                    .equip_break_cost(equip.rare, 0)
                    .is_some_and(|cost| cost.level > 1)
        })
        .unwrap();
    let fodder_config = tables
        .equip
        .iter()
        .find(|equip| equip.is_exp_equip == 1)
        .unwrap();
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (22, 'strengthen', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO currencies (user_id, currency_id, quantity)
         VALUES (22, 3, 100000)",
    )
    .execute(&pool)
    .await
    .unwrap();
    for (uid, equip_id) in [(100_i64, target_config.id), (101_i64, fodder_config.id)] {
        sqlx::query(
            "INSERT INTO equipment
             (uid, user_id, equip_id, level, exp, break_lv, count, is_lock, refine_lv,
              created_at, updated_at)
             VALUES (?, 22, ?, 1, 0, 0, 1, 0, 1, 0, 0)",
        )
        .bind(uid)
        .bind(equip_id)
        .execute(&pool)
        .await
        .unwrap();
    }

    let result = super::strengthen(
        &pool,
        22,
        100,
        vec![EatEquip {
            eat_uid: Some(101),
            count: Some(1),
        }],
    )
    .await
    .unwrap();

    assert_eq!(result.changed_uids, [100]);
    assert_eq!(result.deleted_uids, [101]);
    assert!(result.currency_changes[0].1 < 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM equipment WHERE user_id = 22 AND uid = 101 AND count > 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn break_uses_the_equipment_currency() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let equip = tables.equip.get(1571).unwrap();
    let current = tables.equip_break_cost(equip.rare, 2).unwrap();
    let next = tables.equip_break_cost(equip.rare, 3).unwrap();
    let costs = crate::reward::parse(&next.cost);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (23, 'break', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::game::currencies::add_currency(&pool, 23, 3, next.score_cost)
        .await
        .unwrap();
    for (item_id, count) in &costs.items {
        database::db::game::items::add_item_quantity(&pool, 23, *item_id, *count)
            .await
            .unwrap();
    }
    sqlx::query(
        "INSERT INTO equipment
         (uid, user_id, equip_id, level, exp, break_lv, count, is_lock, refine_lv,
          created_at, updated_at)
         VALUES (200, 23, ?, ?, 1100, 2, 1, 1, 1, 0, 0)",
    )
    .bind(equip.id)
    .bind(current.level)
    .execute(&pool)
    .await
    .unwrap();

    let (_, currency_changes, changed_items, changed_uids) =
        super::break_equip(&pool, 23, 200).await.unwrap();

    assert_eq!(currency_changes, [(3, -next.score_cost)]);
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT quantity FROM currencies WHERE user_id = 23 AND currency_id = 3"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        changed_items,
        costs.items.iter().map(|(id, _)| *id).collect::<Vec<_>>()
    );
    assert_eq!(changed_uids, [200]);
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT break_lv FROM equipment WHERE uid = 200")
            .fetch_one(&pool)
            .await
            .unwrap(),
        3
    );
}
