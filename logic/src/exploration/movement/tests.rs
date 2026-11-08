use super::*;
use database::{db::game::explore::state, models::game::explore::state::MapState};

async fn test_db(saved: &MapState) -> SqlitePool {
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
         VALUES (1, 'explore-move', 0, 0)",
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

fn captured_state() -> MapState {
    let mut saved =
        crate::exploration::snapshot::initial(config::get().explore_map.get(101).unwrap()).unwrap();
    saved.posx = 48;
    saved.posy = 23;
    saved.dir = 270;
    saved.move_nodes = "1#6|48#23".into();
    saved
}

#[tokio::test]
async fn straight_move_persists_the_captured_steps() {
    config::init(&format!(
        "{}/../data/excel2json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let db = test_db(&captured_state()).await;
    let outcome = move_to(&db, 1, 52, 23, None).await.unwrap();
    assert_eq!(
        outcome.steps,
        [
            RoleMove {
                x: 49,
                y: 23,
                direction: 90
            },
            RoleMove {
                x: 50,
                y: 23,
                direction: 90
            },
            RoleMove {
                x: 51,
                y: 23,
                direction: 90
            },
            RoleMove {
                x: 52,
                y: 23,
                direction: 90
            }
        ]
    );
    let mut conn = db.acquire().await.unwrap();
    let saved = state::load(&mut conn, 1, 101).await.unwrap().unwrap();
    assert_eq!((saved.posx, saved.posy, saved.dir), (52, 23, 90));
    assert!(saved.move_nodes.ends_with("49#23|50#23|51#23|52#23"));
    drop(conn);

    assert!(
        move_to(&db, 1, 52, 23, None)
            .await
            .unwrap()
            .steps
            .is_empty()
    );
}

#[tokio::test]
async fn straight_move_preserves_all_captured_directions_and_unique_history() {
    config::init(&format!(
        "{}/../data/excel2json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let mut saved = captured_state();
    saved.move_nodes = "48#23|49#23|50#23".into();
    let db = test_db(&saved).await;

    for (start, target, direction) in [
        ((48, 23), (52, 23), 90),
        ((52, 23), (48, 23), 270),
        ((48, 23), (48, 26), 0),
        ((53, 26), (53, 24), 180),
    ] {
        saved.posx = start.0;
        saved.posy = start.1;
        let mut conn = db.acquire().await.unwrap();
        state::update(&mut conn, 1, &saved).await.unwrap();
        drop(conn);
        let outcome = move_to(&db, 1, target.0, target.1, None).await.unwrap();
        assert!(!outcome.steps.is_empty());
        assert!(outcome.steps.iter().all(|step| step.direction == direction));
        let mut conn = db.acquire().await.unwrap();
        saved = state::load(&mut conn, 1, 101).await.unwrap().unwrap();
    }

    let history = saved.move_nodes.split('|').collect::<Vec<_>>();
    assert_eq!(
        history.len(),
        history.iter().copied().collect::<BTreeSet<_>>().len()
    );
}

#[tokio::test]
async fn move_rejects_unobserved_or_blocked_paths() {
    config::init(&format!(
        "{}/../data/excel2json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let mut saved = captured_state();
    let db = test_db(&saved).await;
    assert!(move_to(&db, 1, 49, 24, None).await.is_err());
    assert!(move_to(&db, 1, 52, 23, Some(440)).await.is_err());
    assert!(move_to(&db, 1, i32::MAX, 23, None).await.is_err());
    let mut conn = db.acquire().await.unwrap();
    assert_eq!(
        state::load(&mut conn, 1, 101).await.unwrap().unwrap(),
        saved
    );
    drop(conn);

    saved.posx = 41;
    saved.posy = 27;
    let mut conn = db.acquire().await.unwrap();
    state::update(&mut conn, 1, &saved).await.unwrap();
    drop(conn);
    assert!(move_to(&db, 1, 41, 28, None).await.is_err());

    let map = config::get().explore_map.get(101).unwrap();
    saved.area_ids = map.areas.keys().copied().collect();
    let blocked = blocked_nodes(map, &saved).unwrap();
    let ((start_x, start_y), (target_x, target_y)) = map
        .nodes
        .iter()
        .find_map(|(&(x, y), node)| {
            [(x + 1, y), (x, y + 1)]
                .into_iter()
                .find(|target| {
                    !blocked.contains(&(x, y))
                        && !blocked.contains(target)
                        && map
                            .nodes
                            .get(target)
                            .is_some_and(|other| other.height != node.height)
                })
                .map(|target| ((x, y), target))
        })
        .unwrap();
    saved.posx = start_x;
    saved.posy = start_y;
    let mut conn = db.acquire().await.unwrap();
    state::update(&mut conn, 1, &saved).await.unwrap();
    drop(conn);
    assert!(move_to(&db, 1, target_x, target_y, None).await.is_err());
}
