use super::*;

async fn test_pool(user_id: i64) -> SqlitePool {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (?, ?, 0, 0)",
    )
    .bind(user_id)
    .bind(format!("investigate-{user_id}"))
    .execute(&pool)
    .await
    .unwrap();
    pool
}

fn box_ids(info: &InvestigateInfo) -> Vec<(i32, Vec<i32>)> {
    info.intel_box
        .iter()
        .map(|group| (group.id.unwrap(), group.clue_ids.clone()))
        .collect()
}

#[tokio::test]
async fn default_projection_has_only_default_clues_and_all_groups() {
    let pool = test_pool(5611).await;
    let info = InvestigateManager::new(5611)
        .info(&pool, config::configs::get())
        .await
        .unwrap()
        .info
        .unwrap();

    assert_eq!(info.clue_ids, vec![11, 41, 51, 61]);
    assert_eq!(
        box_ids(&info),
        vec![
            (1, vec![]),
            (2, vec![]),
            (3, vec![]),
            (4, vec![]),
            (5, vec![]),
            (6, vec![]),
        ]
    );
}

#[tokio::test]
async fn present_dungeon_elements_unlock_all_configured_clues() {
    let pool = test_pool(5612).await;
    let mut expected = Vec::new();
    for (index, clue) in config::configs::get()
        .investigate_clue
        .iter()
        .filter(|clue| clue.map_element != 0)
        .enumerate()
    {
        expected.push(clue.id);
        sqlx::query(
            "INSERT INTO user_dungeon_elements (user_id, element_id, is_finished)
                 VALUES (?, ?, ?)",
        )
        .bind(5612)
        .bind(clue.map_element)
        .bind(index != 0)
        .execute(&pool)
        .await
        .unwrap();
    }
    expected.extend(
        config::configs::get()
            .investigate_clue
            .iter()
            .filter(|clue| clue.default_unlock != 0)
            .map(|clue| clue.id),
    );
    expected.sort_unstable();

    let info = InvestigateManager::new(5612)
        .info(&pool, config::configs::get())
        .await
        .unwrap()
        .info
        .unwrap();

    assert_eq!(info.clue_ids, expected);
    assert_eq!(info.clue_ids.len(), 14);
    assert!(info.intel_box.iter().all(|group| group.clue_ids.is_empty()));
}

#[tokio::test]
async fn put_clue_persists_a_valid_link_once() {
    let pool = test_pool(5613).await;
    let manager = InvestigateManager::new(5613);
    let expected = PutClueReply {
        id: Some(1),
        clue_id: Some(11),
    };

    assert_eq!(
        manager
            .put_clue(&pool, config::configs::get(), 1, 11)
            .await
            .unwrap(),
        expected
    );
    assert_eq!(
        manager
            .put_clue(&pool, config::configs::get(), 1, 11)
            .await
            .unwrap(),
        expected
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM user_investigate_clues
                 WHERE user_id = 5613 AND info_id = 1 AND clue_id = 11",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    let info = InvestigateManager::new(5613)
        .info(&pool, config::configs::get())
        .await
        .unwrap()
        .info
        .unwrap();
    assert_eq!(
        info.intel_box
            .into_iter()
            .find(|group| group.id == Some(1))
            .unwrap()
            .clue_ids,
        vec![11]
    );
}

#[tokio::test]
async fn put_clue_rejects_wrong_group_and_locked_clue() {
    let pool = test_pool(5614).await;
    let manager = InvestigateManager::new(5614);

    assert!(matches!(
        manager.put_clue(&pool, config::configs::get(), 2, 11).await,
        Err(AppError::InvalidRequest)
    ));
    assert!(matches!(
        manager.put_clue(&pool, config::configs::get(), 2, 21).await,
        Err(AppError::InvalidRequest)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM user_investigate_clues WHERE user_id = 5614",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}
