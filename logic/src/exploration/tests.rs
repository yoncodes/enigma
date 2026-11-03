use super::*;
use database::db::game::{explore::state, open_infos};

#[tokio::test]
async fn entry_preserves_progress() {
    config::init(&format!(
        "{}/../data/excel2json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    for scene in config::get().explore_scene.iter() {
        let initial = snapshot::initial(config::get().explore_map.get(scene.id).unwrap()).unwrap();
        assert_eq!(initial.map_id, scene.id);
    }
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&db).await.unwrap();
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (1, 'explore-entry', 0, 0)")
        .execute(&db).await.unwrap();
    let manager = ExplorationManager::new(1);
    assert!(manager.change_map(&db, 101).await.is_err());
    assert!(
        manager
            .simple_info(&db)
            .await
            .unwrap()
            .unlock_map_ids
            .is_empty()
    );
    open_infos::ensure_open_infos(
        &db,
        1,
        &[open_infos::OpenInfoSeed {
            id: 47,
            is_open: true,
        }],
    )
    .await
    .unwrap();
    assert!(manager.info(&db).await.is_err());
    assert!(manager.change_map(&db, -1).await.is_err());
    assert!(manager.change_map(&db, 102).await.is_err());
    let available = manager.simple_info(&db).await.unwrap().unlock_map_ids;
    assert!(available.contains(&101));
    assert!(!available.contains(&102));
    let fresh = manager
        .change_map(&db, 101)
        .await
        .unwrap()
        .explore_info
        .unwrap()
        .explore_map
        .unwrap();
    assert_eq!(
        (fresh.posx, fresh.posy, fresh.dir),
        (Some(1), Some(6), Some(180))
    );
    assert_eq!(fresh.area_ids, vec![1]);
    assert_eq!(fresh.is_first_enter, Some(true));
    let configured = config::get().explore_map.get(101).unwrap();
    assert_eq!(
        fresh.interacts.len(),
        configured
            .areas
            .values()
            .map(|area| area.units.len())
            .sum::<usize>()
    );
    for unit in configured.areas.values().flat_map(|area| &area.units) {
        let actual = fresh
            .interacts
            .iter()
            .find(|value| value.id == Some(unit.id))
            .unwrap();
        assert_eq!(
            (
                actual.r#type,
                actual.status,
                actual.posx,
                actual.posy,
                actual.dir
            ),
            (
                Some(unit.kind),
                Some(unit.interact),
                Some(unit.node_pos[0]),
                Some(unit.node_pos[1]),
                Some(unit.unit_dir)
            )
        );
    }
    let mut conn = db.acquire().await.unwrap();
    let mut saved = state::load(&mut conn, 1, 101).await.unwrap().unwrap();
    let (&(x, y), _) = configured
        .nodes
        .iter()
        .find(|(pos, _)| **pos != (1, 6))
        .unwrap();
    saved.posx = x;
    saved.posy = y;
    saved.dir = 270;
    saved.challenge_count = 2;
    saved
        .area_ids
        .extend(configured.areas.keys().filter(|&&area| area != 1));
    saved.interacts[0].status2 = "{\"saved\":1}".into();
    saved.move_nodes = format!("{x}#{y}");
    sqlx::query(
        "UPDATE user_explore_map_state SET snapshot = ? WHERE user_id = 1 AND map_id = 101",
    )
    .bind(serde_json::to_string(&saved).unwrap())
    .execute(&mut *conn)
    .await
    .unwrap();
    drop(conn);
    let expected = snapshot::info(saved, false);
    let reconnected = ExplorationManager::new(1);
    assert_eq!(
        reconnected.info(&db).await.unwrap().explore_info,
        Some(expected.clone())
    );
    assert_eq!(
        reconnected.change_map(&db, 101).await.unwrap().explore_info,
        Some(expected)
    );
    assert_eq!(
        manager.simple_info(&db).await.unwrap().last_map_id,
        Some(101)
    );
}
