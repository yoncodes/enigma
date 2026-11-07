use super::*;

async fn test_db() -> SqlitePool {
    crate::init_test_server_config();
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
         VALUES (1, 'weekwalk-v2', 0, 0)",
    )
    .execute(&db)
    .await
    .unwrap();
    db
}

#[tokio::test]
async fn info_seeds_captured_season_and_preserves_progress() {
    let db = test_db().await;
    let schedule = &common::config().weekwalk_v2.schedule[0];

    let reply = info_at(&db, 1, schedule.start_time).await.unwrap();
    let info = reply.info.unwrap();
    assert_eq!(info.time_id, Some(21));
    assert_eq!(info.start_time, Some(1_793_527_200_000));
    assert_eq!(info.end_time, Some(1_796_119_200_000));
    assert_eq!(info.pop_rule, Some(true));
    assert!(info.snapshot_infos.is_empty());
    assert_eq!(
        info.layer_infos
            .iter()
            .map(|layer| (layer.id, layer.scene_id, layer.unlock))
            .collect::<Vec<_>>(),
        [
            (Some(1069), Some(41), Some(true)),
            (Some(1070), Some(42), Some(false)),
            (Some(1071), Some(43), Some(false)),
            (Some(1072), Some(44), Some(false)),
        ]
    );
    let expected = [
        (120384100, 400001),
        (120384200, 400002),
        (120384300, 400006),
        (120384400, 400007),
        (120384500, 400011),
        (120384600, 400012),
        (120384700, 400016),
        (120384800, 400017),
    ];
    for (layer_index, layer) in info.layer_infos.iter().enumerate() {
        assert_eq!(layer.all_pass, Some(false));
        assert_eq!(layer.finished, Some(false));
        assert_eq!(layer.show_finished, Some(false));
        assert_eq!(layer.params, None);
        assert_eq!(layer.battle_infos.len(), 2);
        assert_eq!(layer.element_infos.len(), 2);
        for (battle_index, battle) in layer.battle_infos.iter().enumerate() {
            let (battle_id, element_id) = expected[layer_index * 2 + battle_index];
            assert_eq!(battle.battle_id, Some(battle_id));
            assert_eq!(battle.status, Some(0));
            assert!(battle.hero_ids.is_empty());
            assert!(battle.choose_skill_ids.is_empty());
            assert_eq!(battle.hero_group_select, Some(0));
            assert_eq!(battle.element_id, Some(element_id));
            assert!(battle.cup_infos.is_empty());
            assert_eq!(battle.params, None);
            assert_eq!(
                layer.element_infos[battle_index],
                sonettobuf::WeekwalkVer2ElementInfo {
                    element_id: Some(element_id),
                    finish: Some(false),
                    index: Some(battle_index as i32 + 1),
                    visible: Some(battle_index == 0),
                }
            );
        }
    }
    let previous = info.prev_settle.unwrap();
    assert_eq!(previous.max_layer_id, Some(0));
    assert_eq!(previous.max_battle_id, Some(0));
    assert_eq!(previous.max_battle_index, Some(0));
    assert_eq!(previous.show, Some(false));
    assert_eq!(
        previous
            .layer_infos
            .iter()
            .map(|layer| layer.layer_id)
            .collect::<Vec<_>>(),
        [Some(1061), Some(1062), Some(1063), Some(1064)]
    );

    sqlx::query(
        "UPDATE user_weekwalk_v2_layers SET show_finished = 1
         WHERE user_id = 1 AND layer_id = 1069",
    )
    .execute(&db)
    .await
    .unwrap();
    let reloaded = info_at(&db, 1, schedule.start_time)
        .await
        .unwrap()
        .info
        .unwrap();
    assert_eq!(reloaded.layer_infos[0].show_finished, Some(true));
}
