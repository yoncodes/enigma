use super::*;
use flate2::read::GzDecoder;

#[test]
fn team_level_is_absent_when_no_entity_has_a_level() {
    let empty = sonettobuf::FightTeam::default();
    assert_eq!(average_team_level(&empty), None);

    let team = sonettobuf::FightTeam {
        entitys: vec![
            sonettobuf::FightEntityInfo {
                level: Some(10),
                ..Default::default()
            },
            sonettobuf::FightEntityInfo::default(),
            sonettobuf::FightEntityInfo {
                level: Some(20),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    assert_eq!(average_team_level(&team), Some(15));
}

#[test]
fn replay_record_keeps_cloth_and_card_operations_in_the_same_round() {
    let mut active = ActiveBattle::default();
    active
        .pending_cloth_skill_opers
        .push(UseClothSkillOperRecord {
            skill_id: Some(12),
            ..Default::default()
        });

    active.record_round(BeginRoundRequest {
        opers: vec![sonettobuf::BeginRoundOper::default()],
        ..Default::default()
    });

    let records = active.oper_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].cloth_skill_opers[0].skill_id, Some(12));
    assert_eq!(records[0].opers.len(), 1);
    assert!(active.pending_cloth_skill_opers.is_empty());
}

#[test]
fn replay_mode_is_projected_from_the_active_battle() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let active = ActiveBattle {
        is_replay: Some(true),
        runtime: ::battle::engine::runtime::BattleRuntime::new(sonettobuf::Fight {
            is_record: Some(false),
            ..Default::default()
        }),
        ..Default::default()
    };

    assert_eq!(active.start_reply().fight.unwrap().is_record, Some(true));
    assert_eq!(
        active.reconnect_reply().fight.unwrap().is_record,
        Some(true)
    );
}

#[tokio::test]
async fn replay_uses_the_saved_battle_seed() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query("CREATE TABLE dungeon_records (user_id INTEGER, episode_id INTEGER, seed TEXT)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO dungeon_records VALUES (7, 60107, ?)")
        .bind(u64::MAX.to_string())
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        initial_battle_seed(&pool, 7, 60107, true).await.unwrap(),
        u64::MAX
    );
}

#[test]
fn begin_round_steps_use_the_clients_compressed_framing() {
    let reply = compress_round_steps(BeginRoundReply {
        round: Some(sonettobuf::FightRound {
            fight_step: vec![Default::default(), Default::default()],
            ..Default::default()
        }),
    })
    .unwrap();
    let round = reply.round.unwrap();

    assert_eq!(round.total_step, Some(2));
    assert!(round.fight_step.is_empty());

    let compressed = round.fight_step_bytes.unwrap();
    let mut decoder = GzDecoder::new(compressed.as_slice());
    let mut framed = Vec::new();
    decoder.read_to_end(&mut framed).unwrap();
    assert_eq!(framed, [0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn active_fight_reconnects_from_its_fresh_start_checkpoint() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
                    let _ = config::init(&data_dir);
                    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
                    database::run_migrations(&pool).await.unwrap();
                    sqlx::query(
                        "INSERT INTO users (id, username, created_at, updated_at)
                         VALUES (9, 'reconnect', 0, 0)",
                    )
                    .execute(&pool)
                    .await
                    .unwrap();
                    database::db::starter_data::load_all_starter_data(&pool, 9)
                        .await
                        .unwrap();

                    let mut active = ActiveBattle::prepare(
                        &pool,
                        9,
                        10002,
                        1002,
                        StartDungeonRequest {
                            chapter_id: Some(301),
                            episode_id: Some(10002),
                            fight_group: Some(Default::default()),
                            multiplication: Some(1),
                            ..Default::default()
                        },
                    )
                    .await
                    .unwrap();
                    active
                        .activate(&pool, 9, &RewardSet::default())
                        .await
                        .unwrap();
                    let expected = active.reconnect_reply();
                    let expected_start = active.start_reply();
                    let expected_cards = active.card_info_push();
                    active.begin_round(BeginRoundRequest::default()).unwrap();
                    let fight_id = active.fight_id.unwrap();
                    assert!(matches!(
                        BattleState::default().ensure_can_start(&pool, 9).await,
                        Err(AppError::InvalidRequest)
                    ));
                    assert!(
                        battle::create_fight_instance(
                            &pool,
                            battle::NewFightInstance {
                                user_id: 9,
                                episode_id: 10002,
                                battle_id: 1002,
                                multiplication: 1,
                                entry_cost: "{}",
                                checkpoint: "{}",
                                created_at: 0,
                            },
                        )
                        .await
                        .is_err()
                    );

                    let record = battle::load_active_fight(&pool, 9).await.unwrap().unwrap();
                    let restored = ActiveBattle::restore(&pool, 9, record).await.unwrap();

                    assert_eq!(restored.reconnect_reply(), expected);
                    assert_eq!(restored.start_reply(), expected_start);
                    assert_eq!(restored.card_info_push(), expected_cards);
                    assert!(restored.oper_records().is_empty());
                    battle::finish_fight_instance(&pool, 9, fight_id)
                        .await
                        .unwrap();
                    assert!(battle::load_active_fight(&pool, 9).await.unwrap().is_none());
                    BattleState::default()
                        .ensure_can_start(&pool, 9)
                        .await
                        .unwrap();
                });
        })
        .unwrap()
        .join()
        .unwrap();
}

#[tokio::test]
async fn malformed_checkpoint_is_the_only_discardable_restore_error() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let error = ActiveBattle::restore(
        &pool,
        9,
        database::db::game::battle::ActiveFightRecord {
            id: 1,
            episode_id: 10002,
            battle_id: 1002,
            multiplication: 1,
            entry_cost: "{}".into(),
            checkpoint: "{".into(),
        },
    )
    .await
    .unwrap_err();

    assert!(matches!(error, AppError::InvalidBattleCheckpoint(_)));
}

#[tokio::test]
async fn activation_rolls_back_cost_when_an_active_fight_already_exists() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (9, 'activation', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO currencies (user_id, currency_id, quantity, last_recover_time)
         VALUES (9, 4, 5, ?)",
    )
    .bind(common::time::ServerTime::now_ms())
    .execute(&pool)
    .await
    .unwrap();
    let existing_fight = battle::create_fight_instance(
        &pool,
        battle::NewFightInstance {
            user_id: 9,
            episode_id: 10002,
            battle_id: 1002,
            multiplication: 1,
            entry_cost: "{}",
            checkpoint: "{}",
            created_at: 0,
        },
    )
    .await
    .unwrap();

    let mut active = ActiveBattle {
        chapter_id: 301,
        episode_id: 10002,
        battle_id: 1002,
        seed: 7,
        start_request: Some(StartDungeonRequest {
            chapter_id: Some(301),
            episode_id: Some(10002),
            fight_group: Some(Default::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(
        active
            .activate(&pool, 9, &reward::parse("2#4#3"))
            .await
            .is_err()
    );

    let quantity: i32 =
        sqlx::query_scalar("SELECT quantity FROM currencies WHERE user_id = 9 AND currency_id = 4")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(quantity, 5);
    assert!(active.fight_id.is_none());

    battle::finish_fight_instance(&pool, 9, existing_fight)
        .await
        .unwrap();
    active
        .activate(&pool, 9, &reward::parse("2#4#3"))
        .await
        .unwrap();
    let record = battle::load_active_fight(&pool, 9).await.unwrap().unwrap();
    let entry_cost: RewardSet = serde_json::from_str(&record.entry_cost).unwrap();
    assert_eq!(entry_cost.currencies, vec![(4, 3)]);
}
