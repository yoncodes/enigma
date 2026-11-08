use super::*;

async fn database(player_id: i64, username: &str) -> SqlitePool {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, ?, 0, 0)",
    )
    .bind(player_id)
    .bind(username)
    .execute(&db)
    .await
    .unwrap();
    db
}

#[tokio::test]
async fn finish_preserves_best_result_and_reset_only_unlocks_the_team() {
    let db = database(7, "act229").await;
    let heroes = vec![Act229HeroNo {
        hero_id: Some(3143),
        equip_uids: vec![49],
    }];

    let first = finish_act229_battle(&db, 7, 138521, 1, 15, 2, &heroes)
        .await
        .unwrap();
    assert_eq!(first.last_min_round, Some(0));
    let better = finish_act229_battle(&db, 7, 138521, 1, 10, 3, &heroes)
        .await
        .unwrap();
    assert_eq!(better.last_min_round, Some(15));
    act229_heroes_available(&db, 7, 138521, 1, &heroes)
        .await
        .unwrap();
    assert!(
        act229_heroes_available(
            &db,
            7,
            138521,
            1,
            &[Act229HeroNo {
                hero_id: Some(3149),
                equip_uids: vec![50],
            }],
        )
        .await
        .is_err()
    );
    assert!(
        act229_heroes_available(&db, 7, 138521, 2, &heroes)
            .await
            .is_err()
    );

    reset_act229_stage(&db, 7, 138521, 1).await.unwrap();
    let stage = act229_info(&db, 7, Some(138521))
        .await
        .unwrap()
        .stages
        .into_iter()
        .find(|stage| stage.stage_id == Some(1))
        .unwrap();
    assert_eq!(stage.star, Some(0));
    assert_eq!(stage.max_star, Some(3));
    assert_eq!(stage.round, Some(0));
    assert_eq!(stage.min_round, Some(10));
    assert!(stage.heros.is_empty());
    act229_heroes_available(&db, 7, 138521, 2, &heroes)
        .await
        .unwrap();
}

#[tokio::test]
async fn malformed_saved_heroes_do_not_unlock_a_roster() {
    let db = database(42, "act229-corrupt").await;
    sqlx::query(
        "INSERT INTO user_activity229_stages
            (user_id, activity_id, stage_id, heroes_json)
         VALUES (42, 138521, 1, '{')",
    )
    .execute(&db)
    .await
    .unwrap();
    let selected = [Act229HeroNo {
        hero_id: Some(3143),
        equip_uids: Vec::new(),
    }];

    assert!(matches!(
        act229_heroes_available(&db, 42, 138521, 1, &selected).await,
        Err(AppError::Serde(_))
    ));
    assert!(matches!(
        act229_info(&db, 42, Some(138521)).await,
        Err(AppError::Serde(_))
    ));
}

#[tokio::test]
async fn skill_selection_is_configured_persisted_and_locked_after_challenge() {
    let db = database(55, "act229-skill").await;

    let reply = modify_act229_skill(&db, 55, 13917, 1, vec![1006])
        .await
        .unwrap();
    assert_eq!(reply.skill_ids, vec![1006]);
    let stage = act229_info(&db, 55, Some(13917))
        .await
        .unwrap()
        .stages
        .into_iter()
        .find(|stage| stage.stage_id == Some(1))
        .unwrap();
    assert_eq!(stage.skill_ids, vec![1006]);

    assert!(
        modify_act229_skill(&db, 55, 13917, 1, vec![1005])
            .await
            .is_err()
    );
    assert!(
        modify_act229_skill(&db, 55, 13917, 1, vec![1003, 1006])
            .await
            .is_err()
    );

    sqlx::query(
        "UPDATE user_activity229_stages
         SET heroes_json = '[{\"heroId\":3143,\"equipUids\":[]}]'
         WHERE user_id = 55 AND activity_id = 13917 AND stage_id = 1",
    )
    .execute(&db)
    .await
    .unwrap();
    assert!(
        modify_act229_skill(&db, 55, 13917, 1, vec![1003])
            .await
            .is_err()
    );
}

#[tokio::test]
async fn snapshot_updates_touch_the_mapped_stage_team() {
    let db = database(56, "act229-team-touch").await;
    let activity_id = current_activity_id().unwrap();

    let mut tx = db.begin().await.unwrap();
    assert!(
        touch_act229_stage_teams(&mut tx, 56, HeroGroupSnapshotType::AbyssPreset.id(), &[1])
            .await
            .unwrap()
            .is_none()
    );

    let push = touch_act229_stage_teams(&mut tx, 56, HeroGroupSnapshotType::Abyss.id(), &[1])
        .await
        .unwrap()
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(push.activity_id, Some(activity_id));
    assert_eq!(push.stage_sub_ids.len(), 1);
    let stage = &push.stage_sub_ids[0];
    assert_eq!(stage.stage_id, Some(1));
    assert_eq!(stage.hero_group_sub_id, Some(1));
    assert!(stage.last_update_team_time.is_some_and(|time| time > 0));

    let saved = act229_info(&db, 56, Some(activity_id))
        .await
        .unwrap()
        .stages
        .into_iter()
        .find(|stage| stage.stage_id == Some(1))
        .unwrap();
    assert_eq!(saved.hero_group_sub_id, stage.hero_group_sub_id);
    assert_eq!(saved.last_update_team_time, stage.last_update_team_time);
}

#[tokio::test]
async fn stage_team_swap_moves_each_groups_update_time() {
    let db = database(57, "act229-team-swap").await;
    let activity_id = current_activity_id().unwrap();
    for (stage_id, updated_at) in [(1, 11), (2, 22)] {
        sqlx::query(
            "INSERT INTO user_activity229_stages
                (user_id, activity_id, stage_id, hero_group_sub_id, last_update_team_time)
             VALUES (57, ?, ?, ?, ?)",
        )
        .bind(activity_id)
        .bind(stage_id)
        .bind(stage_id)
        .bind(updated_at)
        .execute(&db)
        .await
        .unwrap();
    }
    sqlx::query(
        "UPDATE user_activity229_stages
         SET heroes_json = '[{\"heroId\":3143,\"equipUids\":[]}]'
         WHERE user_id = 57 AND activity_id = ? AND stage_id = 2",
    )
    .bind(activity_id)
    .execute(&db)
    .await
    .unwrap();

    let reply = modify_act229_stage_sub_ids(
        &db,
        57,
        activity_id,
        vec![
            Act229StageTeamSubIdNo {
                stage_id: Some(1),
                hero_group_sub_id: Some(2),
                last_update_team_time: None,
            },
            Act229StageTeamSubIdNo {
                stage_id: Some(2),
                hero_group_sub_id: Some(1),
                last_update_team_time: None,
            },
            Act229StageTeamSubIdNo {
                stage_id: Some(3),
                hero_group_sub_id: Some(3),
                last_update_team_time: None,
            },
        ],
    )
    .await
    .unwrap();
    let stages = reply
        .stages
        .into_iter()
        .map(|stage| (stage.stage_id.unwrap(), stage))
        .collect::<HashMap<_, _>>();
    assert_eq!(stages[&1].hero_group_sub_id, Some(2));
    assert_eq!(stages[&1].last_update_team_time, Some(22));
    assert_eq!(stages[&2].hero_group_sub_id, Some(1));
    assert_eq!(stages[&2].last_update_team_time, Some(11));

    assert!(
        modify_act229_stage_sub_ids(
            &db,
            57,
            activity_id,
            vec![
                Act229StageTeamSubIdNo {
                    stage_id: Some(1),
                    hero_group_sub_id: Some(2),
                    last_update_team_time: None,
                },
                Act229StageTeamSubIdNo {
                    stage_id: Some(2),
                    hero_group_sub_id: Some(3),
                    last_update_team_time: None,
                },
                Act229StageTeamSubIdNo {
                    stage_id: Some(3),
                    hero_group_sub_id: Some(1),
                    last_update_team_time: None,
                },
            ],
        )
        .await
        .is_err()
    );

    assert!(
        modify_act229_stage_sub_ids(
            &db,
            57,
            activity_id,
            vec![
                Act229StageTeamSubIdNo {
                    stage_id: Some(2),
                    hero_group_sub_id: Some(3),
                    last_update_team_time: None,
                },
                Act229StageTeamSubIdNo {
                    stage_id: Some(3),
                    hero_group_sub_id: Some(1),
                    last_update_team_time: None,
                },
                Act229StageTeamSubIdNo {
                    stage_id: Some(1),
                    hero_group_sub_id: Some(2),
                    last_update_team_time: None,
                },
            ],
        )
        .await
        .is_err()
    );

    let reply = modify_act229_stage_sub_ids(
        &db,
        57,
        activity_id,
        vec![Act229StageTeamSubIdNo {
            stage_id: Some(1),
            hero_group_sub_id: Some(4),
            last_update_team_time: None,
        }],
    )
    .await
    .unwrap();
    assert_eq!(
        reply
            .stages
            .into_iter()
            .find(|stage| stage.stage_id == Some(1))
            .unwrap()
            .hero_group_sub_id,
        Some(4)
    );
}
