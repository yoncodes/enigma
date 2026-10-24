use super::*;

#[tokio::test]
async fn incomplete_act128_settlement_persists_score_without_dungeon_completion() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES
         (27, 'act128-failure', 0, 0),
         (28, 'act128-abort', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let fight_id = battle_db::create_fight_instance(
        &pool,
        battle_db::NewFightInstance {
            user_id: 27,
            episode_id: 13500420,
            battle_id: 118353100,
            multiplication: 1,
            entry_cost: "{}",
            checkpoint: "{}",
            created_at: 0,
        },
    )
    .await
    .unwrap();

    let runtime = act128_runtime_with_score(13500420, 118353100, 118353111, 19, 1_000_000);
    assert_eq!(runtime.outcome(), ::battle::BattleOutcome::OutOfRounds);
    let expected_score = runtime.activity_score();
    let active = ActiveBattle {
        fight_id: Some(fight_id),
        chapter_id: 128003,
        episode_id: 13500420,
        battle_id: 118353100,
        runtime,
        ..Default::default()
    };
    settle_failed(&pool, 27, &active, true).await.unwrap();

    let reply = logic::activity::ActivityManager::new(27)
        .act128_info(&pool, Some(138520))
        .await
        .unwrap();
    let boss = reply
        .boss_detail
        .iter()
        .find(|boss| boss.boss_id == Some(2))
        .unwrap();
    assert_eq!(boss.total_point, Some(expected_score));
    assert_eq!(boss.highest_point, Some(expected_score));
    assert!(
        battle_db::load_active_fight(&pool, 27)
            .await
            .unwrap()
            .is_none()
    );

    let abort_fight_id = battle_db::create_fight_instance(
        &pool,
        battle_db::NewFightInstance {
            user_id: 28,
            episode_id: 13500420,
            battle_id: 118353100,
            multiplication: 1,
            entry_cost: "{}",
            checkpoint: "{}",
            created_at: 0,
        },
    )
    .await
    .unwrap();
    let abort_runtime = act128_runtime_with_score(13500420, 118353100, 118353111, 19, 1_000_000);
    let abort_expected_score = abort_runtime.activity_score();
    let abort = ActiveBattle {
        fight_id: Some(abort_fight_id),
        chapter_id: 128003,
        episode_id: 13500420,
        battle_id: 118353100,
        runtime: abort_runtime,
        ..Default::default()
    };
    settle_refund(&pool, 28, &abort, false).await.unwrap();
    let abort_reply = logic::activity::ActivityManager::new(28)
        .act128_info(&pool, Some(138520))
        .await
        .unwrap();
    let abort_boss = abort_reply
        .boss_detail
        .iter()
        .find(|boss| boss.boss_id == Some(2))
        .unwrap();
    assert_eq!(abort_boss.total_point, Some(abort_expected_score));
    assert_eq!(abort_boss.highest_point, Some(abort_expected_score));
}

#[tokio::test]
async fn act128_end_routes_emit_the_current_run_score() {
    use crate::{
        handlers::dungeon::{on_dungeon_end_dungeon, on_fight_end_fight},
        net::{
            app::AppState, context::ConnectionContext, outbound::CommandPacket,
            packet::ClientPacket,
        },
        player::{Player, PlayerState},
    };
    use prost::Message;
    use sonettobuf::{CmdId, EndDungeonPush, EndDungeonRequest, EndFightRequest};
    use tokio::sync::mpsc;

    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let route = configs::get()
        .activity128_battle(13500416, 109380001)
        .unwrap();

    for (player_id, dungeon_route, is_abort) in [
        (37, false, false),
        (38, true, false),
        (41, false, true),
        (42, true, true),
    ] {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        database::run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (?, ?, 0, 0)",
        )
        .bind(player_id)
        .bind(format!("act128-score-route-{player_id}"))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO user_dungeons
                 (user_id, chapter_id, episode_id, star, challenge_count, has_record,
                  left_return_all_num, today_pass_num, today_total_num, created_at, updated_at)
             VALUES (?, 128003, 13500416, 0, 0, 0, 1, 0, 0, 0, 0)",
        )
        .bind(player_id)
        .execute(&pool)
        .await
        .unwrap();
        let fight_id = battle_db::create_fight_instance(
            &pool,
            battle_db::NewFightInstance {
                user_id: player_id,
                episode_id: 13500416,
                battle_id: 109380001,
                multiplication: 1,
                entry_cost: "{}",
                checkpoint: "{}",
                created_at: 0,
            },
        )
        .await
        .unwrap();
        let runtime = act128_runtime_with_score(
            13500416,
            109380001,
            109380001,
            if is_abort { 1 } else { 19 },
            1_000_000,
        );
        assert_eq!(
            runtime.outcome(),
            if is_abort {
                ::battle::BattleOutcome::Unfinished
            } else {
                ::battle::BattleOutcome::OutOfRounds
            }
        );
        let run_score = runtime.activity_score();
        let prior_score = 37;
        let mut tx = pool.begin().await.unwrap();
        logic::activity::settle_act128_score_in_transaction(
            &mut tx,
            player_id,
            13500416,
            109380001,
            prior_score,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let active = ActiveBattle {
            fight_id: Some(fight_id),
            chapter_id: 128003,
            episode_id: 13500416,
            battle_id: 109380001,
            runtime,
            ..Default::default()
        };
        let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
        let (outbound, mut packets) = mpsc::channel(64);
        let mut ctx = ConnectionContext::new(outbound, state);
        ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));
        ctx.player_mut().unwrap().battle.restore_active(active);

        let mut data = Vec::new();
        if dungeon_route {
            EndDungeonRequest {
                is_abort: Some(is_abort),
                ..Default::default()
            }
            .encode(&mut data)
            .unwrap();
            on_dungeon_end_dungeon(
                &mut ctx,
                ClientPacket {
                    sequence: 0,
                    cmd_id: CmdId::DungeonEndDungeonCmd as i16,
                    up_tag: 7,
                    data,
                },
            )
            .await
            .unwrap();
        } else {
            EndFightRequest {
                is_abort: Some(is_abort),
            }
            .encode(&mut data)
            .unwrap();
            on_fight_end_fight(
                &mut ctx,
                ClientPacket {
                    sequence: 0,
                    cmd_id: CmdId::FightEndFightCmd as i16,
                    up_tag: 7,
                    data,
                },
            )
            .await
            .unwrap();
        }

        let score_push = std::iter::from_fn(|| packets.try_recv().ok()).find_map(|packet| {
            let CommandPacket::Push { cmd_id, body, .. } = packet else {
                return None;
            };
            (cmd_id == CmdId::DungeonEndDungeonPushCmd)
                .then(|| EndDungeonPush::decode(&*body).unwrap())
        });
        let score_push = score_push.expect("Activity 128 completion must emit EndDungeonPush");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&score_push.extra_str.unwrap()).unwrap(),
            serde_json::json!({ "score": run_score })
        );

        let info = logic::activity::ActivityManager::new(player_id)
            .act128_info(ctx.state.db, Some(route.activity_id))
            .await
            .unwrap();
        let boss = info
            .boss_detail
            .iter()
            .find(|boss| boss.boss_id == Some(route.boss_id))
            .unwrap();
        assert_eq!(boss.total_point, Some(prior_score + run_score));
        assert_eq!(boss.highest_point, Some(run_score));
    }
}

#[tokio::test]
async fn successful_act128_settlement_includes_the_current_run_score() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (39, 'act128-score-success', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_dungeons
             (user_id, chapter_id, episode_id, star, challenge_count, has_record,
              left_return_all_num, today_pass_num, today_total_num, created_at, updated_at)
         VALUES (39, 128003, 13500416, 0, 0, 0, 1, 0, 0, 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let fight_id = battle_db::create_fight_instance(
        &pool,
        battle_db::NewFightInstance {
            user_id: 39,
            episode_id: 13500416,
            battle_id: 109380001,
            multiplication: 1,
            entry_cost: "{}",
            checkpoint: "{}",
            created_at: 0,
        },
    )
    .await
    .unwrap();
    let runtime = act128_runtime_with_score(13500416, 109380001, 109380001, 1, 1);
    assert_eq!(runtime.outcome(), ::battle::BattleOutcome::Victory);
    let run_score = runtime.activity_score();
    let active = ActiveBattle {
        fight_id: Some(fight_id),
        chapter_id: 128003,
        episode_id: 13500416,
        battle_id: 109380001,
        runtime,
        ..Default::default()
    };

    let settlement = settle_active(
        &pool,
        39,
        &active,
        DungeonCompletion {
            star: 0,
            total_round: active.current_round(),
            multiplier: 1,
            fight_group: None,
        },
        &DungeonRecordStatus::default(),
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&settlement.end_dungeon.extra_str.unwrap())
            .unwrap(),
        serde_json::json!({ "score": run_score })
    );
}

#[tokio::test]
async fn failed_non_act128_route_does_not_emit_a_score_result_push() {
    use crate::{
        handlers::dungeon::on_dungeon_end_dungeon,
        net::{
            app::AppState, context::ConnectionContext, outbound::CommandPacket,
            packet::ClientPacket,
        },
        player::{Player, PlayerState},
    };
    use prost::Message;
    use sonettobuf::{CmdId, EndDungeonRequest, Fight, FightEntityInfo, FightTeam};
    use tokio::sync::mpsc;

    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (40, 'ordinary-failed-route', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::starter_data::load_all_starter_data(&pool, 40)
        .await
        .unwrap();
    let episode = configs::get().episode.get(10001).unwrap();
    let fight_id = battle_db::create_fight_instance(
        &pool,
        battle_db::NewFightInstance {
            user_id: 40,
            episode_id: episode.id,
            battle_id: episode.battle_id,
            multiplication: 1,
            entry_cost: "{}",
            checkpoint: "{}",
            created_at: 0,
        },
    )
    .await
    .unwrap();
    let active = ActiveBattle {
        fight_id: Some(fight_id),
        chapter_id: episode.chapter_id,
        episode_id: episode.id,
        battle_id: episode.battle_id,
        runtime: runtime(Fight {
            episode_id: Some(episode.id),
            battle_id: Some(episode.battle_id),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(32);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(40, PlayerState::new(40, 0)));
    ctx.player_mut().unwrap().battle.restore_active(active);
    let mut data = Vec::new();
    EndDungeonRequest {
        is_abort: Some(false),
        ..Default::default()
    }
    .encode(&mut data)
    .unwrap();

    on_dungeon_end_dungeon(
        &mut ctx,
        ClientPacket {
            sequence: 0,
            cmd_id: CmdId::DungeonEndDungeonCmd as i16,
            up_tag: 7,
            data,
        },
    )
    .await
    .unwrap();

    while let Ok(packet) = packets.try_recv() {
        if let CommandPacket::Push { cmd_id, .. } = packet {
            assert_ne!(cmd_id, CmdId::DungeonEndDungeonPushCmd);
        }
    }
}
