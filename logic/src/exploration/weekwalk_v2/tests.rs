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

#[tokio::test]
async fn mark_finished_shown_persists_the_captured_layer_flag() {
    let db = test_db().await;
    let schedule = &common::config().weekwalk_v2.schedule[0];

    assert!(
        mark_finished_shown_at(&db, 1, 1069, schedule.start_time)
            .await
            .is_err()
    );
    sqlx::query(
        "UPDATE user_weekwalk_v2_layers SET all_pass = 1, finished = 1
         WHERE user_id = 1 AND layer_id = 1069",
    )
    .execute(&db)
    .await
    .unwrap();
    let reply = mark_finished_shown_at(&db, 1, 1069, schedule.start_time)
        .await
        .unwrap();
    assert_eq!(reply.layer_id, Some(1069));
    let info = info_at(&db, 1, schedule.start_time)
        .await
        .unwrap()
        .info
        .unwrap();
    assert_eq!(info.layer_infos[0].show_finished, Some(true));

    assert!(
        mark_finished_shown_at(&db, 1, 9999, schedule.start_time)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn change_group_persists_an_owned_group_for_the_captured_battle() {
    let db = test_db().await;
    let schedule = &common::config().weekwalk_v2.schedule[0];
    sqlx::query(
        "INSERT INTO hero_groups_common
         (user_id, group_id, created_at, updated_at) VALUES (1, 3, 0, 0)",
    )
    .execute(&db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (2, 'other-weekwalk-player', 0, 0)",
    )
    .execute(&db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO hero_groups_common
         (user_id, group_id, created_at, updated_at) VALUES (2, 4, 0, 0)",
    )
    .execute(&db)
    .await
    .unwrap();

    let reply = change_group_at(&db, 1, 1069, 120384100, 3, schedule.start_time)
        .await
        .unwrap();
    assert_eq!(reply.layer_id, Some(1069));
    assert_eq!(reply.battle_id, Some(120384100));
    assert_eq!(reply.select, Some(3));
    let info = info_at(&db, 1, schedule.start_time)
        .await
        .unwrap()
        .info
        .unwrap();
    assert_eq!(
        info.layer_infos[0].battle_infos[0].hero_group_select,
        Some(3)
    );

    assert!(
        change_group_at(&db, 1, 1069, 120384100, 4, schedule.start_time)
            .await
            .is_err()
    );
    assert!(
        change_group_at(&db, 1, 1070, 120384300, 3, schedule.start_time)
            .await
            .is_err()
    );
    assert!(
        change_group_at(&db, 1, 1069, 120384200, 3, schedule.start_time)
            .await
            .is_err()
    );
    sqlx::query(
        "UPDATE user_weekwalk_v2_battles SET status = 1
         WHERE user_id = 1 AND layer_id = 1069 AND battle_id = 120384100",
    )
    .execute(&db)
    .await
    .unwrap();
    assert!(
        change_group_at(&db, 1, 1069, 120384100, 3, schedule.start_time)
            .await
            .is_err()
    );
    assert!(
        change_group_at(&db, 1, 1069, 9999, 3, schedule.start_time)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn choose_skill_persists_one_unlocked_skill_for_an_owned_group() {
    let db = test_db().await;
    let schedule = &common::config().weekwalk_v2.schedule[0];
    sqlx::query(
        "INSERT INTO hero_groups_common
         (user_id, group_id, created_at, updated_at) VALUES (1, 3, 0, 0)",
    )
    .execute(&db)
    .await
    .unwrap();

    let reply = choose_skill_at(&db, 1, 3, vec![2017], schedule.start_time)
        .await
        .unwrap();
    assert_eq!(reply.no, Some(3));
    assert_eq!(reply.skill_ids, [2017]);
    let info = info_at(&db, 1, schedule.start_time)
        .await
        .unwrap()
        .info
        .unwrap();
    assert_eq!(info.snapshot_infos[0].no, Some(3));
    assert_eq!(info.snapshot_infos[0].skill_ids, [2017]);

    assert!(
        choose_skill_at(&db, 1, 3, vec![2033], schedule.start_time)
            .await
            .is_err()
    );
    assert!(
        choose_skill_at(&db, 1, 3, vec![2017, 2019], schedule.start_time)
            .await
            .is_err()
    );
    assert!(
        choose_skill_at(&db, 1, 4, vec![2017], schedule.start_time)
            .await
            .is_err()
    );
    sqlx::query(
        "UPDATE user_weekwalk_v2_layers SET unlock = 1
         WHERE user_id = 1 AND layer_id = 1071",
    )
    .execute(&db)
    .await
    .unwrap();
    let later = choose_skill_at(&db, 1, 3, vec![2033], schedule.start_time)
        .await
        .unwrap();
    assert_eq!(later.skill_ids, [2033]);

    let cleared = choose_skill_at(&db, 1, 3, vec![], schedule.start_time)
        .await
        .unwrap();
    assert!(cleared.skill_ids.is_empty());
    let info = info_at(&db, 1, schedule.start_time)
        .await
        .unwrap()
        .info
        .unwrap();
    assert!(info.snapshot_infos[0].skill_ids.is_empty());
}
