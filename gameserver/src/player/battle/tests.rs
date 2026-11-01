use super::*;
use flate2::read::GzDecoder;

fn runtime(fight: sonettobuf::Fight) -> ::battle::Battle {
    ::battle::tooling::unstarted_battle(
        ::battle::catalog::BattleCatalog::new(config::configs::get()),
        fight,
    )
}

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
    active.pending_cloth_skill_opers.push(UseClothSkillRequest {
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
        runtime: runtime(sonettobuf::Fight {
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
fn active_fight_reconnects_from_its_committed_checkpoint() {
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
                            fight_group: Some(sonettobuf::FightGroup {
                                cloth_id: Some(7),
                                ..Default::default()
                            }),
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
                    let fight_id = active.fight_id.unwrap();
                    let mut state = BattleState::default();
                    state.restore_active(active);

                    let before = format!("{:?}", state.active_snapshot());
                    let checkpoint_before = battle::load_active_fight(&pool, 9)
                        .await
                        .unwrap()
                        .unwrap()
                        .checkpoint;
                    let mut rejected = state.active_snapshot().unwrap();
                    rejected.multiplication = Some(2);
                    rejected.fight_id = Some(fight_id + 1);
                    assert!(state.commit_active(&pool, 9, rejected).await.is_err());
                    assert_eq!(format!("{:?}", state.active_snapshot()), before);
                    assert_eq!(
                        battle::load_active_fight(&pool, 9)
                            .await
                            .unwrap()
                            .unwrap()
                            .checkpoint,
                        checkpoint_before
                    );

                    let mut candidate = state.active_snapshot().unwrap();
                    candidate
                        .use_cloth_skill(UseClothSkillRequest {
                            skill_id: Some(30010701),
                            from_id: Some(0),
                            r#type: Some(0),
                            ..Default::default()
                        })
                        .unwrap();
                    state.commit_active(&pool, 9, candidate).await.unwrap();

                    let mut candidate = state.active_snapshot().unwrap();
                    candidate.begin_round(BeginRoundRequest::default()).unwrap();
                    state.commit_active(&pool, 9, candidate).await.unwrap();

                    let mut candidate = state.active_snapshot().unwrap();
                    candidate.begin_round(BeginRoundRequest::default()).unwrap();
                    state.commit_active(&pool, 9, candidate).await.unwrap();

                    let active = state.active_snapshot().unwrap();
                    let expected = active.reconnect_reply();
                    let expected_start = active.start_reply();
                    let expected_cards = active.card_info_push();
                    let expected_records = active.oper_records();
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
                    assert_eq!(restored.oper_records(), expected_records);
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
    let legacy: BattleCheckpoint = serde_json::from_str(
        r#"{
            "chapter_id": 301,
            "start_request": {},
            "seed": 7,
            "tower_context": null,
            "act229_context": null
        }"#,
    )
    .unwrap();
    assert!(legacy.rounds.is_empty());
    assert!(legacy.pending_cloth_skill_opers.is_empty());

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


#[test]
#[ignore = "requires an isolated user-provided battle database"]
fn diagnostic_replay_saved_unfinished_tune_and_action() {
    std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
            use std::{str::FromStr, time::Instant};
            use sqlx::sqlite::SqliteConnectOptions;
            let data = std::env::var("ENIGMA_BATTLE_DATA_DIR").unwrap();
            let _ = config::init(&data);
            let path = std::env::var("ENIGMA_DIAGNOSTIC_DB").unwrap();
            let options = SqliteConnectOptions::from_str(&format!("sqlite:{path}"))
                .unwrap().read_only(true).create_if_missing(false);
            let pool = SqlitePool::connect_with(options).await.unwrap();
            let player_id: i64 = sqlx::query_scalar("SELECT user_id FROM fight_instances WHERE active = 1 ORDER BY id DESC LIMIT 1")
                .fetch_one(&pool).await.unwrap();
            let mut record = battle::load_active_fight(&pool, player_id).await.unwrap().unwrap();
            let mut checkpoint: BattleCheckpoint = serde_json::from_str(&record.checkpoint).unwrap();
            let round = checkpoint.rounds[0].clone();
            checkpoint.rounds.clear(); checkpoint.pending_cloth_skill_opers.clear();
            record.checkpoint = serde_json::to_string(&checkpoint).unwrap();
            let start = Instant::now();
            let mut active = ActiveBattle::restore(&pool, player_id, record).await.unwrap();
            println!("restore_opening_ms={}", start.elapsed().as_millis());
            let show = |label: &str, active: &ActiveBattle| {
                let cards = active.card_info_push().card_group;
                println!("{label}: {:?}", cards.iter().map(|c| (c.uid, c.skill_id, c.energy)).collect::<Vec<_>>());
            };
            show("opening", &active);
            for request in &round.cloth_skill_opers {
                let start = Instant::now();
                let (reply, _) = active.use_cloth_skill(*request).unwrap();
                println!("cloth_ms={} steps={:?}", start.elapsed().as_millis(), reply.round.unwrap().fight_step.iter()
                    .flat_map(|step| &step.act_effect).map(|effect| (effect.effect_type,effect.reserve_str.clone())).collect::<Vec<_>>());
                show("after_cloth", &active);
            }
            let start = Instant::now();
            let result = active.begin_round(round.request.clone());
            println!("begin_round_ms={} result={:?}", start.elapsed().as_millis(), result.as_ref().map(|reply| {
                reply.round.as_ref().map(|round| (round.total_step, round.fight_step_bytes.as_ref().map(Vec::len)))
            }));
            result.unwrap();
            show("after_round", &active);
        });
    }).unwrap().join().unwrap();
}

#[test]
#[ignore = "requires an isolated read-only copy of the player's database"]
fn diagnostic_saved_party_battle_start() {
    std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
            use std::{str::FromStr, time::Instant};
            use sqlx::sqlite::SqliteConnectOptions;
            let _ = config::init(&std::env::var("ENIGMA_BATTLE_DATA_DIR").unwrap());
            let path = std::env::var("ENIGMA_DIAGNOSTIC_DB").unwrap();
            let options = SqliteConnectOptions::from_str(&format!("sqlite:{path}"))
                .unwrap().read_only(true).create_if_missing(false);
            let pool = SqlitePool::connect_with(options).await.unwrap();
            let heroes = [20000077_i64, 20000080, 20000081, 20000067];
            let equips = [30000026_i64, 30000035, 30000036, 30000016];
            let group = sonettobuf::FightGroup { hero_list: heroes.to_vec(), cloth_id: Some(6),
                equips: heroes.iter().zip(equips).map(|(&uid,equip)| sonettobuf::FightEquip {
                    hero_uid: Some(uid), equip_uid: vec![equip], ..Default::default()
                }).collect(), ..Default::default() };
            for (episode,battle) in [(3810103,119381010), (13500416,109380001)] {
                let request = StartDungeonRequest { episode_id: Some(episode),
                    fight_group: Some(group.clone()), ..Default::default() };
                let t=Instant::now();
                let result = ActiveBattle::prepare(&pool, 4313302, episode, battle, request).await;
                println!("party_start episode={episode} battle={battle} elapsed_ms={} result={:?}",
                    t.elapsed().as_millis(), result.as_ref().map(|active| {
                        (active.start_reply().fight.as_ref().and_then(|f| f.attacker.as_ref())
                            .map(|team| team.entitys.len()), active.card_info_push().card_group.len())
                    }));
                assert!(result.is_ok(), "party startup failed");
            }
        });
    }).unwrap().join().unwrap();
}
