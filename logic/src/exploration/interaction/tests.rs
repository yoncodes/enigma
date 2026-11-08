use super::*;
use database::models::game::explore::state::MapState;

fn captured_state() -> MapState {
    config::init(&format!(
        "{}/../data/excel2json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    crate::exploration::snapshot::initial(config::get().explore_map.get(101).unwrap()).unwrap()
}

async fn test_db(saved: &MapState) -> SqlitePool {
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (1, 'explore-interact', 0, 0)",
    )
    .execute(&db)
    .await
    .unwrap();
    let mut conn = db.acquire().await.unwrap();
    state::insert(&mut conn, 1, saved).await.unwrap();
    state::select(&mut conn, 1, saved.map_id).await.unwrap();
    drop(conn);
    db
}

#[tokio::test]
async fn captured_teleports_commit_destination_and_facing() {
    for (id, start, destination, direction) in [
        (440, (41, 27), (41, 39), 0),
        (402, (39, 48), (41, 57), 0),
        (414, (41, 57), (41, 27), 180),
    ] {
        let mut saved = captured_state();
        saved.posx = start.0;
        saved.posy = start.1;
        saved.move_nodes = format!("{}#{}", start.0, start.1);
        let db = test_db(&saved).await;

        assert_eq!(
            interact(&db, 1, id, None).await.unwrap(),
            InteractOutcome {
                id,
                action: InteractAction::Transfer {
                    x: destination.0,
                    y: destination.1,
                    direction,
                },
            }
        );
        let mut conn = db.acquire().await.unwrap();
        let saved = state::load(&mut conn, 1, 101).await.unwrap().unwrap();
        assert_eq!(
            (saved.posx, saved.posy, saved.dir),
            (destination.0, destination.1, direction)
        );
        assert!(
            saved
                .move_nodes
                .ends_with(&format!("{}#{}", destination.0, destination.1))
        );
    }
}

#[tokio::test]
async fn teleport_rejects_wrong_side_and_parameters_without_mutation() {
    let mut saved = captured_state();
    saved.posx = 41;
    saved.posy = 29;
    let db = test_db(&saved).await;

    assert!(interact(&db, 1, 440, None).await.is_err());
    assert!(interact(&db, 1, 440, Some(String::new())).await.is_err());
    let mut conn = db.acquire().await.unwrap();
    assert_eq!(state::load(&mut conn, 1, 101).await.unwrap(), Some(saved));
}

#[tokio::test]
async fn teleport_rejects_unsupported_directions_without_mutation() {
    config::init(&format!(
        "{}/../data/excel2json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let mut horizontal =
        crate::exploration::snapshot::initial(config::get().explore_map.get(102).unwrap()).unwrap();
    horizontal.posx = -7;
    horizontal.posy = 20;
    horizontal
        .interacts
        .iter_mut()
        .find(|state| state.id == 459)
        .unwrap()
        .status = 3;
    let db = test_db(&horizontal).await;
    assert!(interact(&db, 1, 459, None).await.is_err());
    let mut conn = db.acquire().await.unwrap();
    assert_eq!(
        state::load(&mut conn, 1, 102).await.unwrap(),
        Some(horizontal)
    );

    let map = config::get().explore_map.get(101).unwrap();
    let unit = config::exploration::Unit {
        id: 1,
        kind: 213,
        node_pos: [1, 6],
        area_id: 1,
        interact: 3,
        unit_dir: 0,
        trigger_dir: 0,
        offset_size: [0; 4],
        default_walkable: true,
        trigger_by_click: true,
        trigger_effects: vec![],
        done_effects: vec![],
        special_datas: vec![Value::String("1#6".into())],
    };
    let mut any = captured_state();
    let before = any.clone();
    assert!(transfer(map, &unit, &mut any).is_err());
    assert_eq!(any, before);
}

#[tokio::test]
async fn teleport_rejects_invalid_interaction_state_without_mutation() {
    let mut saved = captured_state();
    saved.posx = 41;
    saved.posy = 27;
    let db = test_db(&saved).await;

    for status in [1, 11] {
        saved
            .interacts
            .iter_mut()
            .find(|state| state.id == 440)
            .unwrap()
            .status = status;
        let mut conn = db.acquire().await.unwrap();
        state::update(&mut conn, 1, &saved).await.unwrap();
        drop(conn);
        assert!(interact(&db, 1, 440, None).await.is_err());
        let mut conn = db.acquire().await.unwrap();
        assert_eq!(
            state::load(&mut conn, 1, 101).await.unwrap(),
            Some(saved.clone())
        );
    }

    let valid = captured_state()
        .interacts
        .into_iter()
        .find(|state| state.id == 440)
        .unwrap();
    for variant in 0..3 {
        saved.interacts.retain(|state| state.id != 440);
        match variant {
            0 => {}
            1 => {
                let mut stale = valid.clone();
                stale.kind = 212;
                saved.interacts.push(stale);
            }
            2 => {
                saved.interacts.push(valid.clone());
                saved.interacts.push(valid.clone());
            }
            _ => unreachable!(),
        }
        let mut conn = db.acquire().await.unwrap();
        state::update(&mut conn, 1, &saved).await.unwrap();
        drop(conn);
        assert!(interact(&db, 1, 440, None).await.is_err());
        let mut conn = db.acquire().await.unwrap();
        assert_eq!(
            state::load(&mut conn, 1, 101).await.unwrap(),
            Some(saved.clone())
        );
    }
}

#[tokio::test]
async fn captured_rune_activates_the_selected_item() {
    let mut saved = captured_state();
    saved.posx = 32;
    saved.posy = 17;
    saved
        .interacts
        .iter_mut()
        .find(|state| state.id == 209)
        .unwrap()
        .status = 7;
    saved.items.push(ItemState {
        uid: 77,
        item_id: 150002,
        quantity: 1,
        status: 0,
    });
    saved.use_item_uid = 77;
    let db = test_db(&saved).await;

    let changed = ItemState {
        status: 1,
        ..saved.items[0].clone()
    };
    assert_eq!(
        interact(&db, 1, 209, None).await.unwrap(),
        InteractOutcome {
            id: 209,
            action: InteractAction::ItemChanged(changed.clone()),
        }
    );
    let mut conn = db.acquire().await.unwrap();
    let persisted = state::load(&mut conn, 1, 101).await.unwrap().unwrap();
    assert_eq!(persisted.items, [changed]);
    assert_eq!(persisted.use_item_uid, 77);
    assert_eq!(
        persisted
            .interacts
            .iter()
            .find(|state| state.id == 209)
            .unwrap()
            .status,
        7
    );
}

#[tokio::test]
async fn rune_rejects_unproven_item_states() {
    let mut saved = captured_state();
    saved.posx = 32;
    saved.posy = 17;
    saved
        .interacts
        .iter_mut()
        .find(|state| state.id == 209)
        .unwrap()
        .status = 7;
    saved.items.push(ItemState {
        uid: 77,
        item_id: 150002,
        quantity: 1,
        status: 0,
    });
    let db = test_db(&saved).await;

    assert!(interact(&db, 1, 209, None).await.is_err());
    for (item_id, quantity, status) in [(150001, 1, 0), (150002, 0, 0), (150002, 1, 1)] {
        saved.use_item_uid = 77;
        saved.items[0].item_id = item_id;
        saved.items[0].quantity = quantity;
        saved.items[0].status = status;
        let mut conn = db.acquire().await.unwrap();
        state::update(&mut conn, 1, &saved).await.unwrap();
        drop(conn);
        assert!(interact(&db, 1, 209, None).await.is_err());
    }
}
