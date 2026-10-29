use super::*;

#[tokio::test]
async fn first_clear_uses_the_configured_first_battle() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let episode = configs::get().episode.get(10103).unwrap();

    assert_eq!(episode_battle_id(&pool, 1, episode).await.unwrap(), 11021);

    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (1, 'first', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_dungeons
             (user_id, chapter_id, episode_id, star, challenge_count, has_record,
              left_return_all_num, today_pass_num, today_total_num, created_at, updated_at)
             VALUES (1, 101, 10103, 1, 0, 0, 1, 0, 0, 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(episode_battle_id(&pool, 1, episode).await.unwrap(), 1102);
    let episode_without_first_battle = configs::get().episode.get(10001).unwrap();
    assert_eq!(
        episode_battle_id(&pool, 1, episode_without_first_battle)
            .await
            .unwrap(),
        1001
    );
}

#[test]
fn saved_start_matches_with_or_without_the_restart_marker() {
    let start = sonettobuf::StartDungeonRequest {
        chapter_id: Some(301),
        episode_id: Some(10002),
        fight_group: Some(Default::default()),
        multiplication: Some(1),
        ..Default::default()
    };
    let active = ActiveBattle {
        start_request: Some(start.clone()),
        ..Default::default()
    };
    let mut restart = start;

    assert!(matches_saved_dungeon_start(&active, &restart));
    restart.is_restart = Some(true);
    assert!(matches_saved_dungeon_start(&active, &restart));
    restart.episode_id = Some(10003);
    assert!(!matches_saved_dungeon_start(&active, &restart));

    restart.episode_id = Some(10002);
    let tower = ActiveBattle {
        start_request: active.start_request.clone(),
        tower_context: Some(crate::logic::battle_setup::tower::BattleContext {
            tower_type: 1,
            tower_id: 2,
            layer_id: 3,
            difficulty: 4,
            talent_plan_id: 5,
        }),
        ..Default::default()
    };
    assert!(!matches_saved_dungeon_start(&tower, &restart));
}

#[test]
fn abort_push_carries_the_abort_result_and_required_fight_group() {
    let push = abort_end_fight(&ActiveBattle {
        fight_id: Some(42),
        fight_group: Some(Default::default()),
        is_replay: Some(false),
        ..Default::default()
    });

    let record = push.record.unwrap();
    assert_eq!(record.fight_result, Some(-1));
    assert_eq!(record.fight_name.as_deref(), Some(""));
    assert!(record.fight_time.is_some());
    assert!(push.fight_group_a.is_some());
    assert_eq!(push.is_record, Some(false));
}

#[tokio::test]
async fn instruction_open_refreshes_only_after_new_state() {
    use crate::{
        handlers::dungeon::on_instruction_dungeon_open,
        net::{
            app::AppState, context::ConnectionContext, outbound::CommandPacket,
            packet::ClientPacket,
        },
        player::{Player, PlayerState},
    };
    use prost::Message;
    use sonettobuf::{CmdId, InstructionDungeonOpenRequest};
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
         VALUES (30, 'instruction-open', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(4);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(30, PlayerState::new(30, 0)));
    let open_id = configs::get()
        .instruction_level
        .iter()
        .find(|level| level.pre_episode == 0)
        .unwrap()
        .episode_id;
    let mut data = Vec::new();
    InstructionDungeonOpenRequest {
        open_id: vec![open_id],
    }
    .encode(&mut data)
    .unwrap();

    on_instruction_dungeon_open(
        &mut ctx,
        ClientPacket {
            sequence: 0,
            cmd_id: CmdId::InstructionDungeonOpenCmd as i16,
            up_tag: 7,
            data,
        },
    )
    .await
    .unwrap();

    let CommandPacket::Push {
        cmd_id: CmdId::DungeonInstructionDungeonInfoPushCmd,
        body,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("new open did not refresh instruction state");
    };
    let push = sonettobuf::InstructionDungeonInfoPush::decode(&*body).unwrap();
    assert!(push.open_ids.contains(&open_id));
    assert!(matches!(
        packets.try_recv().unwrap(),
        CommandPacket::Reply {
            cmd_id: CmdId::InstructionDungeonOpenCmd,
            ..
        }
    ));

    let mut data = Vec::new();
    InstructionDungeonOpenRequest::default()
        .encode(&mut data)
        .unwrap();
    on_instruction_dungeon_open(
        &mut ctx,
        ClientPacket {
            sequence: 0,
            cmd_id: CmdId::InstructionDungeonOpenCmd as i16,
            up_tag: 8,
            data,
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        packets.try_recv().unwrap(),
        CommandPacket::Reply {
            cmd_id: CmdId::InstructionDungeonOpenCmd,
            ..
        }
    ));
    assert!(packets.try_recv().is_err());
}

#[tokio::test]
async fn dungeon_abort_sends_terminal_fight_push_before_reply() {
    use crate::{
        handlers::dungeon::on_dungeon_end_dungeon,
        net::{
            app::AppState, context::ConnectionContext, outbound::CommandPacket,
            packet::ClientPacket,
        },
        player::{Player, PlayerState},
    };
    use prost::Message;
    use sonettobuf::CmdId;
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
         VALUES (29, 'abort-push', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::starter_data::load_all_starter_data(&pool, 29)
        .await
        .unwrap();

    let episode = configs::get().episode.get(10001).unwrap();
    let fight_id = battle_db::create_fight_instance(
        &pool,
        battle_db::NewFightInstance {
            user_id: 29,
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
        fight_group: Some(Default::default()),
        ..Default::default()
    };

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(32);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(29, PlayerState::new(29, 0)));
    ctx.player_mut().unwrap().battle.restore_active(active);

    let mut data = Vec::new();
    sonettobuf::EndDungeonRequest {
        is_abort: Some(true),
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

    let mut end_dungeon = None;
    let mut end_fight = None;
    let mut reply = None;
    let mut position = 0;
    while let Ok(packet) = packets.try_recv() {
        match packet {
            CommandPacket::Push {
                cmd_id: CmdId::DungeonEndDungeonPushCmd,
                ..
            } => {
                end_dungeon = Some(position);
            }
            CommandPacket::Push {
                cmd_id: CmdId::FightEndFightPushCmd,
                body,
                ..
            } => {
                let push = sonettobuf::EndFightPush::decode(&*body).unwrap();
                assert_eq!(
                    push.record.unwrap().fight_result,
                    Some(FightResult::Abort as i32)
                );
                end_fight = Some(position);
            }
            CommandPacket::Reply {
                cmd_id: CmdId::DungeonEndDungeonCmd,
                ..
            } => {
                reply = Some(position);
            }
            _ => {}
        }
        position += 1;
    }

    let end_dungeon = end_dungeon.unwrap();
    let end_fight = end_fight.unwrap();
    let reply = reply.unwrap();
    assert!(end_dungeon < end_fight);
    assert!(end_fight < reply);
    assert!(!ctx.player().unwrap().battle.has_active());
}

#[tokio::test]
async fn completed_tutorial_sends_tasks_before_room_unlock() {
    use crate::{
        handlers::dungeon::send_completed_dungeon,
        net::{app::AppState, context::ConnectionContext, outbound::CommandPacket},
        player::{Player, PlayerState},
    };
    use database::models::game::{block_packages::BlockPackage, buildings::Building};
    use logic::reward::AppliedRewards;
    use sonettobuf::{CmdId, DungeonUpdatePush, EndDungeonPush, OpenInfo};
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
         VALUES (34, 'room-order', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::starter_data::load_all_starter_data(&pool, 34)
        .await
        .unwrap();

    let episode = configs::get().episode.get(10003).unwrap();
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(64);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(34, PlayerState::new(34, 0)));

    send_completed_dungeon(
        &mut ctx,
        34,
        episode.chapter_id,
        episode.id,
        DungeonSettlement {
            kill_count: 0,
            hero_ids: Vec::new(),
            rewards: AppliedRewards {
                block_packages: vec![BlockPackage {
                    user_id: 34,
                    block_package_id: 6,
                    unused_block_ids: "[]".into(),
                    used_block_ids: "[]".into(),
                }],
                room_buildings: vec![Building {
                    uid: 1,
                    user_id: 34,
                    define_id: 22201,
                    in_use: false,
                    x: 0,
                    y: 0,
                    rotate: 0,
                    level: 1,
                    created_at: 0,
                    updated_at: 0,
                }],
                ..Default::default()
            },
            dungeon_update: DungeonUpdatePush::default(),
            open_infos: vec![OpenInfo::default()],
            end_dungeon: EndDungeonPush::default(),
            compose_push: None,
            tasks: Vec::new(),
        },
    )
    .await
    .unwrap();

    let mut relevant = Vec::new();
    while let Ok(packet) = packets.try_recv() {
        if let Some(cmd_id) = match packet {
            CommandPacket::Push { cmd_id, .. }
                if matches!(
                    cmd_id,
                    CmdId::UpdateTaskPushCmd
                        | CmdId::UpdateOpenPushCmd
                        | CmdId::BlockPackageGainPushCmd
                        | CmdId::BuildingGainPushCmd
                        | CmdId::DungeonUpdatePushCmd
                        | CmdId::DungeonEndDungeonPushCmd
                ) =>
            {
                Some(cmd_id)
            }
            _ => None,
        } {
            relevant.push(cmd_id);
        }
    }

    let task_count = relevant
        .iter()
        .take_while(|cmd_id| **cmd_id == CmdId::UpdateTaskPushCmd)
        .count();
    assert!(task_count > 0);
    assert_eq!(
        &relevant[task_count..],
        [
            CmdId::UpdateOpenPushCmd,
            CmdId::BlockPackageGainPushCmd,
            CmdId::BuildingGainPushCmd,
            CmdId::DungeonUpdatePushCmd,
            CmdId::DungeonEndDungeonPushCmd,
        ]
    );
}

#[tokio::test]
async fn abort_dungeon_keeps_saved_progress_but_reports_no_new_star() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (18, 'abort', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let active = ActiveBattle {
        chapter_id: 9000,
        episode_id: 90002501,
        runtime: runtime(sonettobuf::Fight {
            cur_round: Some(15),
            ..Default::default()
        }),
        ..Default::default()
    };

    let (update, end) = abort_dungeon_updates(&pool, 18, &active).await.unwrap();
    assert_eq!(update.dungeon_info.unwrap().star, Some(0));
    assert_eq!(end.star, Some(0));

    sqlx::query(
        "INSERT INTO user_dungeons
             (user_id, chapter_id, episode_id, star, challenge_count, has_record,
              left_return_all_num, today_pass_num, today_total_num, created_at, updated_at)
             VALUES (18, 9000, 90002501, 2, 0, 0, 1, 0, 0, 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let (update, end) = abort_dungeon_updates(&pool, 18, &active).await.unwrap();

    assert_eq!(update.dungeon_info.unwrap().star, Some(2));
    assert_eq!(end.star, Some(0));
    assert_eq!(end.total_round, Some(15));
}

#[tokio::test]
async fn tutorial_trial_settlement_creates_its_first_progress_row() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (28, 'trial-settlement', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::starter_data::load_all_starter_data(&pool, 28)
        .await
        .unwrap();

    let episode = configs::get().episode.get(110101).unwrap();
    let fight_id = battle_db::create_fight_instance(
        &pool,
        battle_db::NewFightInstance {
            user_id: 28,
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
        fight_group: Some(sonettobuf::FightGroup {
            hero_list: vec![-3, -2, -1],
            ..Default::default()
        }),
        ..Default::default()
    };

    let settlement = settle_active(
        &pool,
        28,
        &active,
        DungeonCompletion {
            star: 1,
            total_round: 1,
            multiplier: 1,
            fight_group: active.fight_group.as_ref(),
        },
        &DungeonRecordStatus::default(),
    )
    .await
    .unwrap();

    assert_eq!(
        settlement.dungeon_update.dungeon_info.unwrap().star,
        Some(1)
    );
    assert_eq!(
        dungeons::episode_star(&pool, 28, episode.id).await.unwrap(),
        1
    );
}

#[tokio::test]
async fn unlimited_dungeon_clear_keeps_unrelated_daily_counters() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (19, 'clear', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_chapter_type_nums
             (user_id, chapter_type, today_pass_num, today_total_num, last_reset_date)
             VALUES (19, 6, 1, 2, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let (dungeon, counters, _) = dungeons::update_dungeon_progress(&pool, 19, 9040, 90400101, 1)
        .await
        .unwrap();

    assert_eq!(dungeon.challenge_count, 0);
    assert_eq!(dungeon.left_return_all_num, 1);
    assert_eq!((dungeon.today_pass_num, dungeon.today_total_num), (0, 0));
    assert_eq!(counters.len(), 1);
    assert_eq!(counters[0].chapter_type, 6);
    assert_eq!(
        (counters[0].today_pass_num, counters[0].today_total_num),
        (1, 2)
    );
    let (reply, rows) = dungeon_info(&pool, 19).await.unwrap();
    assert_eq!(reply.dungeon_info_size, Some(rows.len() as i32));
}

#[tokio::test]
async fn dungeon_unlocks_apply_prerequisites_rewards_and_tutorials_once() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (20, 'unlock', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::starter_data::load_all_starter_data(&pool, 20)
        .await
        .unwrap();
    assert_eq!(
        dungeons::get_reward_points(&pool, 20)
            .await
            .unwrap()
            .into_iter()
            .find(|points| points.chapter_id == 0)
            .unwrap()
            .reward_point,
        0
    );
    sqlx::query(
        "INSERT INTO user_dungeons
            (user_id, chapter_id, episode_id, star, challenge_count, has_record,
             left_return_all_num, today_pass_num, today_total_num, created_at, updated_at)
         VALUES (20, 102, 10215, 1, 0, 0, 1, 0, 0, 123, 123)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_dungeon_reward_repairs (user_id, episode_id, star)
         VALUES (20, 10215, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let guide_steps_before = (
        database::db::game::guides::get_guide_progress(&pool, 20, 101)
            .await
            .unwrap()
            .map(|progress| progress.step_id),
        database::db::game::guides::get_guide_progress(&pool, 20, 103)
            .await
            .unwrap()
            .map(|progress| progress.step_id),
    );
    let unlocked = crate::logic::dungeon::DungeonManager::new(20)
        .unlock_chapter(&pool, 103)
        .await
        .unwrap();
    assert!(
        unlocked
            .episodes
            .iter()
            .filter_map(|episode| episode.dungeon.as_ref())
            .all(|dungeon| {
                dungeon.challenge_count == 0
                    && dungeon.left_return_all_num == 1
                    && dungeon.today_pass_num == 0
            })
    );
    assert!(
        unlocked
            .episodes
            .iter()
            .any(|episode| episode.rewards.player_info_changed)
    );
    assert!(
        unlocked
            .episodes
            .iter()
            .any(|episode| !episode.material_changes.is_empty())
    );
    assert!(
        unlocked
            .episodes
            .iter()
            .any(|episode| !episode.rewards.cloth_updates.is_empty())
    );
    assert!(!unlocked.trails.finished_element_ids.is_empty());
    let finished_elements = dungeons::get_finished_elements(&pool, 20).await.unwrap();
    assert!(
        unlocked
            .trails
            .finished_element_ids
            .iter()
            .all(|element_id| finished_elements.contains(element_id))
    );
    let expected_reward_points = unlocked
        .trails
        .finished_element_ids
        .iter()
        .map(|element_id| {
            config::configs::get()
                .chapter_map_element
                .get(*element_id)
                .unwrap()
                .reward_point
        })
        .sum::<i32>();
    assert_eq!(
        dungeons::get_reward_points(&pool, 20)
            .await
            .unwrap()
            .into_iter()
            .find(|points| points.chapter_id == 0)
            .unwrap()
            .reward_point,
        expected_reward_points
    );
    for episode_id in [10001, 10002, 10003, 10101, 10215, 10315] {
        assert!(dungeons::episode_star(&pool, 20, episode_id).await.unwrap() > 0);
    }
    assert_eq!(dungeons::episode_star(&pool, 20, 10316).await.unwrap(), 0);
    assert!(
        sqlx::query_scalar::<_, Option<i64>>(
            "SELECT repaired_at FROM user_dungeon_reward_repairs
             WHERE user_id = 20 AND episode_id = 10215",
        )
        .fetch_one(&pool)
        .await
        .unwrap()
        .is_some()
    );
    assert!(
        sqlx::query_scalar::<_, i32>("SELECT exp FROM users WHERE id = 20")
            .fetch_one(&pool)
            .await
            .unwrap()
            > 80
    );
    let repeated = crate::logic::dungeon::DungeonManager::new(20)
        .unlock_chapter(&pool, 103)
        .await
        .unwrap();
    assert!(!repeated.changed);
    assert!(repeated.episodes.is_empty());
    assert!(repeated.trails.finished_element_ids.is_empty());
    assert_eq!(
        dungeons::get_reward_points(&pool, 20)
            .await
            .unwrap()
            .into_iter()
            .find(|points| points.chapter_id == 0)
            .unwrap()
            .reward_point,
        expected_reward_points
    );

    let story_only_episode = config::configs::get()
        .story_episodes()
        .find(|episode| episode.chapter_id <= 103 && episode.battle_id == 0)
        .unwrap();
    assert_eq!(
        dungeons::episode_star(&pool, 20, story_only_episode.id)
            .await
            .unwrap(),
        1
    );
    let guide_steps_after = (
        database::db::game::guides::get_guide_progress(&pool, 20, 101)
            .await
            .unwrap()
            .map(|progress| progress.step_id),
        database::db::game::guides::get_guide_progress(&pool, 20, 103)
            .await
            .unwrap()
            .map(|progress| progress.step_id),
    );
    assert_eq!(guide_steps_after, guide_steps_before);

    let trail_gated_stage = 10402;
    crate::logic::dungeon::DungeonManager::new(20)
        .unlock_stage(&pool, trail_gated_stage)
        .await
        .unwrap();
    assert_eq!(
        dungeons::episode_star(&pool, 20, trail_gated_stage)
            .await
            .unwrap(),
        0
    );
    assert!(
        dungeons::get_finished_elements(&pool, 20)
            .await
            .unwrap()
            .contains(&1040101)
    );
    assert!(
        dungeons::can_start_episode(&pool, 20, 104, trail_gated_stage)
            .await
            .unwrap()
    );

    crate::logic::dungeon::DungeonManager::new(20)
        .unlock_chapter(&pool, 401)
        .await
        .unwrap();
    assert!(dungeons::episode_star(&pool, 20, 40105).await.unwrap() > 0);
    assert!(dungeons::episode_star(&pool, 20, 420).await.unwrap() > 0);
    assert_eq!(dungeons::episode_star(&pool, 20, 40106).await.unwrap(), 0);
    assert!(
        dungeons::can_start_episode(&pool, 20, 401, 40106)
            .await
            .unwrap()
    );

    let selected_stage = config::configs::get()
        .resource_episodes()
        .find(|episode| episode.chapter_id == 501)
        .unwrap()
        .id;
    crate::logic::dungeon::DungeonManager::new(20)
        .unlock_stage(&pool, selected_stage)
        .await
        .unwrap();
    assert_eq!(
        dungeons::episode_star(&pool, 20, selected_stage)
            .await
            .unwrap(),
        0
    );
    assert!(
        dungeons::can_start_episode(&pool, 20, 501, selected_stage)
            .await
            .unwrap()
    );
}

#[test]
fn normal_push_uses_the_runtime_outcome() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let active = ActiveBattle {
        fight_id: Some(42),
        fight_group: Some(Default::default()),
        runtime: runtime(sonettobuf::Fight {
            attacker: Some(sonettobuf::FightTeam {
                entitys: vec![sonettobuf::FightEntityInfo {
                    uid: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(sonettobuf::FightTeam {
                entitys: vec![sonettobuf::FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(0),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    assert_eq!(
        completed_end_fight(&active).record.unwrap().fight_result,
        Some(FightResult::Succ as i32)
    );
}
