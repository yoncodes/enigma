use super::*;

async fn test_pool(user_id: i64) -> SqlitePool {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
        .bind(user_id)
        .bind(format!("dungeon-{user_id}"))
        .execute(&pool)
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn map_element_completion_persists_record_without_replaying_rewards() {
    let pool = test_pool(31).await;
    let element = config::configs::get()
        .chapter_map_element
        .iter()
        .find(|element| element.reward.is_empty() && element.reward_point == 0)
        .unwrap();
    sqlx::query(
        "INSERT INTO user_dungeon_elements (user_id, element_id, is_finished)
         VALUES (31, ?, 0)",
    )
    .bind(element.id)
    .execute(&pool)
    .await
    .unwrap();

    let manager = DungeonManager::new(31);
    let completion = manager
        .complete_map_element(&pool, element.id, vec![1, 2], "choice=2".into())
        .await
        .unwrap();
    assert_eq!(completion.reply.element_id, Some(element.id));
    assert_eq!(
        manager
            .map_element_records(&pool, vec![element.id])
            .await
            .unwrap()
            .record_infos[0]
            .record
            .as_deref(),
        Some("choice=2")
    );
    assert!(
        manager
            .complete_map_element(&pool, element.id, Vec::new(), String::new())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn finished_map_element_unlocks_its_configured_dependent() {
    let pool = test_pool(32).await;
    let (element, prerequisite) = config::configs::get()
        .chapter_map_element
        .iter()
        .find_map(|element| {
            element
                .condition
                .strip_prefix("ChapterMapElement=")
                .and_then(|id| id.parse::<i32>().ok())
                .map(|id| (element, id))
        })
        .unwrap();
    sqlx::query("INSERT INTO user_dungeon_maps (user_id, map_id) VALUES (32, ?)")
        .bind(element.map_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO user_dungeon_elements (user_id, element_id, is_finished)
         VALUES (32, ?, 1)",
    )
    .bind(prerequisite)
    .execute(&pool)
    .await
    .unwrap();

    let (_, unlocked) = dungeons::reconcile_map_progression(&pool, 32)
        .await
        .unwrap();
    assert!(unlocked.contains(&element.id));
}

#[tokio::test]
async fn completed_episode_unlocks_maps_that_reference_its_chain_alias() {
    let pool = test_pool(34).await;
    sqlx::query("INSERT INTO user_dungeon_maps (user_id, map_id) VALUES (34, 10404)")
        .execute(&pool)
        .await
        .unwrap();

    for (chapter_id, episode_id, expected_map_id) in [(104, 10404, 10405), (105, 10502, 10503)] {
        sqlx::query(
            "INSERT INTO user_dungeons
                (user_id, chapter_id, episode_id, star, created_at, updated_at)
             VALUES (34, ?, ?, 1, 0, 0)",
        )
        .bind(chapter_id)
        .bind(episode_id)
        .execute(&pool)
        .await
        .unwrap();

        let (maps, elements) = dungeons::reconcile_map_progression(&pool, 34)
            .await
            .unwrap();
        assert!(maps.contains(&expected_map_id));
        if episode_id == 10404 {
            assert!(elements.contains(&1040401));
        }
    }
}

#[tokio::test]
async fn chapter_unlock_reports_missing_reward_character() {
    let pool = test_pool(33).await;
    let error = match DungeonManager::new(33).unlock_chapter(&pool, 113).await {
        Ok(_) => panic!("chapter 113 contains unavailable hero 3154"),
        Err(error) => error,
    };

    assert_eq!(
        error.to_string(),
        "Custom error: chapter 113 rewards unavailable heroes [3154]"
    );
}

#[test]
fn final_initial_tutorial_battle_grants_room_inventory_outside_bonus_lists() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let tables = config::configs::get();
    let episode = tables
        .episode
        .get(tables.initial_tutorial_final_episode().unwrap())
        .unwrap();

    let completion = completion_rewards(episode, true, 0, 2, 1);

    assert_eq!(completion.rewards.block_packages, [(6, 1)]);
    assert_eq!(completion.rewards.room_buildings, [(22201, 1)]);
    assert!(completion.first_bonus.is_empty());
    assert!(completion.normal_bonus.is_empty());
    assert!(completion.advanced_bonus.is_empty());
}

#[test]
fn breakthrough_repeat_clear_grants_configured_normal_rewards() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let episode = config::configs::get().episode.get(70104).unwrap();

    let completion = completion_rewards(episode, false, 2, 2, 1);

    assert_eq!(completion.player_exp, 0);
    assert!(completion.first_bonus.is_empty());
    assert!(completion.normal_bonus.contains(&(1, 115012, 1)));
    assert!(completion.normal_bonus.contains(&(2, 3, 240)));
    assert!(completion.normal_bonus.contains(&(3, 0, 240)));
}
