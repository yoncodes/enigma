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
