use super::*;

#[tokio::test]
async fn mark_show_finished_persists_the_client_flag() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (1, 'weekwalk-mark', 0, 0)",
    )
    .execute(&db)
    .await
    .unwrap();
    let manager = WeekwalkManager::new(1);
    let map_id = manager.info(&db).await.unwrap().info.unwrap().map_info[0]
        .id
        .unwrap();
    sqlx::query(
        "UPDATE user_weekwalk_maps SET is_show_finished = TRUE
         WHERE user_id = 1 AND map_id = ?",
    )
    .bind(map_id)
    .execute(&db)
    .await
    .unwrap();

    assert_eq!(
        manager.mark_show_finished(&db, map_id).await.unwrap(),
        MarkShowFinishedReply {
            layer_id: Some(map_id)
        }
    );
    let (_, maps) = weekwalk::get_weekwalk_info(&db, 1).await.unwrap();
    assert!(!maps[0].is_show_finished.unwrap());
    assert!(manager.mark_show_finished(&db, i32::MAX).await.is_err());
}

#[test]
fn current_cycle_maps_each_configured_layer_to_its_encounters() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let cycle = current_cycle(tables).unwrap();
    assert!(!cycle.maps.is_empty());
    assert!(
        cycle
            .maps
            .windows(2)
            .all(|maps| maps[0].layer < maps[1].layer)
    );
    assert_eq!(cycle.maps.first().unwrap().layer, 1);

    for map in &cycle.maps {
        let config = tables.weekwalk.get(map.map_id).unwrap();
        assert_eq!(
            config.issue_id,
            if map.layer < 11 { 0 } else { cycle.issue_id }
        );
        assert!(!map.encounters.is_empty());
        for encounter in &map.encounters {
            let element = tables.weekwalk_element.get(encounter.element_id).unwrap();
            if map.layer < 11 {
                assert_eq!(cycle::battle_step(element).unwrap().1, encounter.battle_id);
            } else if element.round_id == 1 {
                assert_eq!(encounter.battle_id, config.fight_id_front);
            } else {
                assert_eq!(element.round_id, 2);
                assert_eq!(encounter.battle_id, config.fight_id_rear);
            }
        }
    }

    for (map_id, expected) in [
        (
            101,
            vec![
                (800101, 10112),
                (800102, 10113),
                (800103, 10115),
                (800104, 10114),
                (800105, 10117),
            ],
        ),
        (
            201,
            vec![
                (800601, 10121),
                (800602, 10124),
                (800603, 10126),
                (800604, 10125),
                (800605, 10129),
            ],
        ),
        (
            205,
            vec![
                (801001, 40314),
                (801002, 40315),
                (801003, 40316),
                (801004, 40317),
                (801005, 40318),
            ],
        ),
    ] {
        let encounters = &cycle
            .maps
            .iter()
            .find(|map| map.map_id == map_id)
            .unwrap()
            .encounters;
        assert_eq!(
            encounters
                .iter()
                .map(|encounter| (encounter.battle_id, encounter.element_id))
                .collect::<Vec<_>>(),
            expected
        );
    }
}
