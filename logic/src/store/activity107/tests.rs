use super::*;

async fn buy_test(db: &SqlitePool, user: i64, activity: i32, goods: i32, count: i32, now: i64)
    -> Result<reward::RewardedReply<Buy107GoodsReply>, AppError> {
    let schedule = [common::config::ActivitySchedule { id: 13803, start_time: 1_790_244_000_000,
        end_time: 1_793_613_599_000, is_unlock: true }];
    buy_with_schedule(db, user, activity, goods, count, now, &schedule).await
}

async fn setup() -> SqlitePool {
    crate::init_test_server_config();
    let data = std::env::var("ENIGMA_BATTLE_DATA_DIR").unwrap_or_else(|_| format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR")));
    let _ = config::init(&data);
    let db = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&db).await.unwrap();
    for (id, name, balance) in [(970101, "store-one", 10_730), (970102, "store-two", 500), (970103, "store-failure", 2_000)] {
        sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
            .bind(id).bind(name).execute(&db).await.unwrap();
        sqlx::query("INSERT INTO currencies (user_id, currency_id, quantity) VALUES (?, 3801, ?)")
            .bind(id).bind(balance).execute(&db).await.unwrap();
    }
    db
}

async fn snapshot(db: &SqlitePool, user: i64) -> (i32, i32, i32) {
    let balance: i32 = sqlx::query_scalar("SELECT quantity FROM currencies WHERE user_id = ? AND currency_id = 3801")
        .bind(user).fetch_one(db).await.unwrap();
    let item: Option<i32> = sqlx::query_scalar("SELECT quantity FROM items WHERE user_id = ? AND item_id = 140001")
        .bind(user).fetch_optional(db).await.unwrap();
    let count: Option<i32> = sqlx::query_scalar("SELECT state FROM user_activity_state WHERE user_id = ? AND activity_id = 13803 AND kind = ? AND entry_id = 104")
        .bind(user).bind(ActivityStateKind::Act107Goods.id()).fetch_optional(db).await.unwrap();
    (balance, item.unwrap_or(0), count.unwrap_or(0))
}

#[tokio::test]
async fn activity107_shop_charges_delivers_and_persists_limit_atomically() {
    let db = setup().await;
    let user = 970101;
    let at = 1_793_000_000_000;
    let info = infos(&db, user, 13803).await.unwrap();
    assert_eq!(info.goods_infos.iter().find(|g| g.id == Some(104)).unwrap().buy_count, Some(0));

    for expected in 1..=2 {
        let result = buy_test(&db, user, 13803, 104, 1, at).await.unwrap();
        assert_eq!(result.reply.goods_info.unwrap().buy_count, Some(expected));
        assert_eq!(result.material_changes, vec![(1, 140001, 1)]);
        assert_eq!(result.rewards.currency_ids, vec![(3801, -1000)]);
        assert_eq!(snapshot(&db, user).await, (10_730 - 1000 * expected, expected, expected));
    }
    assert!(buy_test(&db, user, 13803, 104, 1, at).await.is_err());
    assert_eq!(snapshot(&db, user).await, (8_730, 2, 2));
    let info = infos(&db, user, 13803).await.unwrap();
    assert_eq!(info.goods_infos.iter().find(|g| g.id == Some(104)).unwrap().buy_count, Some(2));

    assert!(buy_test(&db, 970102, 13803, 104, 1, at).await.is_err());
    assert_eq!(snapshot(&db, 970102).await, (500, 0, 0));
    sqlx::query("CREATE TRIGGER reject_test_item BEFORE INSERT ON items WHEN NEW.user_id = 970103 BEGIN SELECT RAISE(ABORT, 'injected delivery failure'); END")
        .execute(&db).await.unwrap();
    assert!(buy_test(&db, 970103, 13803, 104, 1, at).await.is_err());
    assert_eq!(snapshot(&db, 970103).await, (2_000, 0, 0));
    sqlx::query("DROP TRIGGER reject_test_item").execute(&db).await.unwrap();
    buy_test(&db, 970103, 13803, 104, 1, at).await.unwrap();
    assert_eq!(snapshot(&db, 970103).await, (1_000, 1, 1));
    for (act, id, count, time) in [(13803, 104, 0, at), (13803, 104, -1, at),
        (13803, 104, i32::MAX, at), (13803, 999999, 1, at),
        (13803, 103, 1, 1_793_613_600_000), (13801, 104, 1, at)] {
        assert!(buy_test(&db, user, act, id, count, time).await.is_err());
    }
    assert_eq!(snapshot(&db, user).await, (8_730, 2, 2));

    // The equipment in the screenshot must be delivered through the equipment inventory.
    let equip = buy_test(&db, user, 13803, 101, 1, at).await.unwrap();
    assert_eq!(equip.reply.goods_info.unwrap().buy_count, Some(1));
    assert_eq!(equip.rewards.equip_uids.len(), 1);
    assert_eq!(snapshot(&db, user).await, (730, 2, 2));
}

#[test]
fn activity107_shop_rejects_unknown_product_type_and_overflow() {
    assert!(reward::parse_strict("31#490001#1").is_err());
    let mut costs = reward::parse_strict("2#3801#18000").unwrap();
    assert!(scale_checked(&mut costs, i32::MAX).is_err());
}
