use super::*;
use database::models::game::explore::state::{ItemState, MapState};

async fn test_db() -> SqlitePool {
    config::init(&format!(
        "{}/../data/excel2json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (1, 'explore-item', 0, 0)",
    )
    .execute(&db)
    .await
    .unwrap();
    let mut saved =
        crate::exploration::snapshot::initial(config::get().explore_map.get(101).unwrap()).unwrap();
    saved.items.push(ItemState {
        uid: 77,
        item_id: 150002,
        quantity: 1,
        status: 0,
    });
    let mut conn = db.acquire().await.unwrap();
    database::db::game::explore::state::insert(&mut conn, 1, &saved)
        .await
        .unwrap();
    database::db::game::explore::state::select(&mut conn, 1, 101)
        .await
        .unwrap();
    drop(conn);
    db
}

#[tokio::test]
async fn active_item_selection_toggles_and_persists() {
    let db = test_db().await;
    let selected = use_item(&db, 1, 77, 0, 0, None).await.unwrap();
    assert_eq!(
        selected,
        UseItemOutcome {
            uid: 77,
            selected_uid: 77,
            deselected_item: None,
        }
    );
    let deselected = use_item(&db, 1, 77, 0, 0, None).await.unwrap();
    assert_eq!(deselected.selected_uid, 0);
    assert_eq!(deselected.deselected_item.unwrap().uid, 77);
    let mut conn = db.acquire().await.unwrap();
    assert_eq!(
        database::db::game::explore::state::load(&mut conn, 1, 101)
            .await
            .unwrap()
            .unwrap()
            .use_item_uid,
        0
    );
}

#[tokio::test]
async fn item_selection_rejects_uncaptured_variants() {
    let db = test_db().await;
    assert!(use_item(&db, 1, 0, 0, 0, None).await.is_err());
    assert!(use_item(&db, 1, 78, 0, 0, None).await.is_err());
    assert!(use_item(&db, 1, 77, 1, 0, None).await.is_err());
    assert!(use_item(&db, 1, 77, 0, 1, None).await.is_err());
    assert!(use_item(&db, 1, 77, 0, 0, Some(209)).await.is_err());

    let mut conn = db.acquire().await.unwrap();
    let mut saved = database::db::game::explore::state::load(&mut conn, 1, 101)
        .await
        .unwrap()
        .unwrap();
    saved.items[0].quantity = 0;
    database::db::game::explore::state::update(&mut conn, 1, &saved)
        .await
        .unwrap();
    drop(conn);
    assert!(use_item(&db, 1, 77, 0, 0, None).await.is_err());

    let mut conn = db.acquire().await.unwrap();
    saved.items[0].quantity = 1;
    saved.items[0].status = 0;
    saved.items[0].item_id = 150002;
    saved.items.push(ItemState {
        uid: 77,
        item_id: 150002,
        quantity: 0,
        status: 1,
    });
    database::db::game::explore::state::update(&mut conn, 1, &saved)
        .await
        .unwrap();
    drop(conn);
    assert!(use_item(&db, 1, 77, 0, 0, None).await.is_err());

    let mut conn = db.acquire().await.unwrap();
    saved.items.pop();
    saved.items[0].quantity = 1;
    saved.items[0].item_id = config::get()
        .explore_item
        .iter()
        .find(|item| item.effect != "2")
        .unwrap()
        .id;
    database::db::game::explore::state::update(&mut conn, 1, &saved)
        .await
        .unwrap();
    drop(conn);
    assert!(use_item(&db, 1, 77, 0, 0, None).await.is_err());
}

#[test]
fn snapshot_projects_persisted_items_and_selection() {
    config::init(&format!(
        "{}/../data/excel2json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let mut saved: MapState =
        crate::exploration::snapshot::initial(config::get().explore_map.get(101).unwrap()).unwrap();
    saved.items.push(ItemState {
        uid: 77,
        item_id: 150002,
        quantity: 1,
        status: 1,
    });
    saved.use_item_uid = 77;
    let info = crate::exploration::snapshot::info(saved, false);
    assert_eq!(info.use_item_uid, Some(77));
    assert_eq!(
        info.explore_items,
        [sonettobuf::ExploreItemNo {
            uid: Some(77),
            item_id: Some(150002),
            quantity: Some(1),
            status: Some(1),
        }]
    );
}
