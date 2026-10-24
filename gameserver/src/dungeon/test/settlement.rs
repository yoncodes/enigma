use super::*;

#[tokio::test]
async fn seedless_record_does_not_block_its_first_replayable_replacement() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO dungeon_records
         (user_id, episode_id, record_round, hero_list, sub_hero_list, cloth_id, equips, version, created_at, oper_records)
         VALUES (9, 10101, 1, '[]', '[]', 1, '[]', 7, 0, '[]')",
    )
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(
        dungeons::dungeon_record_round(&pool, 9, 10101)
            .await
            .unwrap(),
        None
    );

    let seed = u64::MAX - 9;
    let record = dungeons::prepare_dungeon_record(&pool, 9, 7, seed, &Default::default(), &[])
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(
        dungeons::save_prepared_dungeon_record_if_faster_in_transaction(
            &mut tx, 9, 10101, 5, &record,
        )
        .await
        .unwrap()
    );
    tx.commit().await.unwrap();
    assert_eq!(
        dungeons::load_dungeon_record_seed(&pool, 9, 10101)
            .await
            .unwrap(),
        Some(seed)
    );
}

#[tokio::test]
async fn battleless_settlement_rolls_back_its_entry_cost_on_failure() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (24, 'battleless', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::starter_data::load_all_starter_data(&pool, 24)
        .await
        .unwrap();
    sqlx::query("UPDATE currencies SET quantity = 100 WHERE user_id = 24 AND currency_id = 4")
        .execute(&pool)
        .await
        .unwrap();
    let episode = configs::get().episode.get(1000002).unwrap();
    let currency_before: i32 = sqlx::query_scalar(
        "SELECT quantity FROM currencies WHERE user_id = 24 AND currency_id = 4",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let result = settle_battleless(
        &pool,
        24,
        -1,
        episode.id,
        DungeonCompletion {
            star: 1,
            total_round: 0,
            multiplier: 1,
            fight_group: None,
        },
        &Default::default(),
    )
    .await;

    assert!(result.is_err());
    let currency_after: i32 = sqlx::query_scalar(
        "SELECT quantity FROM currencies WHERE user_id = 24 AND currency_id = 4",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(currency_after, currency_before);

    settle_battleless(
        &pool,
        24,
        episode.chapter_id,
        episode.id,
        DungeonCompletion {
            star: 1,
            total_round: 0,
            multiplier: 1,
            fight_group: None,
        },
        &Default::default(),
    )
    .await
    .unwrap();
    let currency_after_commit: i32 = sqlx::query_scalar(
        "SELECT quantity FROM currencies WHERE user_id = 24 AND currency_id = 4",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(currency_after_commit, currency_before - 6);

    assert!(
        settle_battleless(
            &pool,
            24,
            episode.chapter_id,
            episode.id,
            DungeonCompletion {
                star: 1,
                total_round: 0,
                multiplier: 1,
                fight_group: None,
            },
            &Default::default(),
        )
        .await
        .is_err()
    );
    let currency_after_retry: i32 = sqlx::query_scalar(
        "SELECT quantity FROM currencies WHERE user_id = 24 AND currency_id = 4",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(currency_after_retry, currency_after_commit);
}

#[tokio::test]
async fn refund_commits_with_active_fight_finalization() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (25, 'refund', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::starter_data::load_all_starter_data(&pool, 25)
        .await
        .unwrap();
    let episode = configs::get().episode.get(10101).unwrap();
    let fight_id = battle_db::create_fight_instance(
        &pool,
        battle_db::NewFightInstance {
            user_id: 25,
            episode_id: episode.id,
            battle_id: episode.battle_id,
            multiplication: 2,
            entry_cost: "{}",
            checkpoint: "{}",
            created_at: 0,
        },
    )
    .await
    .unwrap();
    let currency_before: i32 = sqlx::query_scalar(
        "SELECT quantity FROM currencies WHERE user_id = 25 AND currency_id = 4",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let stored = battle_db::load_active_fight(&pool, 25)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.multiplication, 2);
    let active = ActiveBattle {
        fight_id: Some(fight_id + 1),
        episode_id: episode.id,
        multiplication: Some(stored.multiplication),
        ..Default::default()
    };

    assert!(settle_refund(&pool, 25, &active, false).await.is_err());
    let currency_after_failure: i32 = sqlx::query_scalar(
        "SELECT quantity FROM currencies WHERE user_id = 25 AND currency_id = 4",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(currency_after_failure, currency_before);
    assert!(
        battle_db::load_active_fight(&pool, 25)
            .await
            .unwrap()
            .is_some()
    );

    settle_checkpoint_refund(&pool, 25, fight_id, reward::parse("2#4#16"))
        .await
        .unwrap();
    let currency_after_commit: i32 = sqlx::query_scalar(
        "SELECT quantity FROM currencies WHERE user_id = 25 AND currency_id = 4",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(currency_after_commit, currency_before + 16);
    assert!(
        battle_db::load_active_fight(&pool, 25)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn point_reward_claim_uses_shared_progress_and_is_idempotent() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (26, 'point-reward', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::starter_data::load_all_starter_data(&pool, 26)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE user_dungeon_reward_points
         SET reward_point = 10
         WHERE user_id = 26 AND chapter_id = 0",
    )
    .execute(&pool)
    .await
    .unwrap();
    let currency_before: i32 = sqlx::query_scalar(
        "SELECT quantity FROM currencies WHERE user_id = 26 AND currency_id = 2",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let claim = logic::dungeon::DungeonManager::new(26)
        .claim_point_rewards(&pool, vec![1])
        .await
        .unwrap();

    assert_eq!(claim.reply.id, vec![1]);
    let currency_after: i32 = sqlx::query_scalar(
        "SELECT quantity FROM currencies WHERE user_id = 26 AND currency_id = 2",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(currency_after, currency_before + 50);
    let points = dungeons::get_reward_points(&pool, 26).await.unwrap();
    assert_eq!(points[0].has_get_point_reward_ids, vec![1]);
    assert!(
        logic::dungeon::DungeonManager::new(26)
            .claim_point_rewards(&pool, vec![1])
            .await
            .is_err()
    );
    let currency_after_retry: i32 = sqlx::query_scalar(
        "SELECT quantity FROM currencies WHERE user_id = 26 AND currency_id = 2",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(currency_after_retry, currency_after);
}
