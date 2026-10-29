use super::*;
use sonettobuf::{
    BeforeStartWeekwalkBattleReply, BeforeStartWeekwalkBattleRequest, EndDungeonReply,
    EndDungeonRequest, EndFightReply, EndFightRequest, Fight, FightEntityInfo, FightTeam,
    GetWeekwalkInfoReply, GetWeekwalkInfoRequest, StartDungeonReply, StartDungeonRequest,
    WeekwalkInfoUpdatePush,
};

fn victory_runtime(battle_id: i32) -> ::battle::Battle {
    ::battle::tooling::unstarted_battle(
        ::battle::catalog::BattleCatalog::new(configs::get()),
        Fight {
            battle_id: Some(battle_id),
            cur_round: Some(1),
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
                    current_hp: Some(0),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        },
    )
}

#[tokio::test]
async fn weekwalk_info_routes_into_a_valid_prebattle_selection() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 920;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'weekwalk-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    database::db::starter_data::load_all_starter_data(&pool, player_id)
        .await
        .unwrap();
    let hero_list: Vec<i64> =
        sqlx::query_scalar("SELECT uid FROM heroes WHERE user_id = ? ORDER BY uid LIMIT 4")
            .bind(player_id)
            .fetch_all(&pool)
            .await
            .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(8);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    let mut data = Vec::new();
    GetWeekwalkInfoRequest {}.encode(&mut data).unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::GetWeekwalkInfoCmd as i16,
            up_tag: 51,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();
    let CommandPacket::Reply {
        cmd_id: CmdId::GetWeekwalkInfoCmd,
        body,
        result_code: 0,
        up_tag: 51,
        ..
    } = packets.recv().await.unwrap()
    else {
        panic!("Weekwalk info did not reach its handler");
    };
    let info = GetWeekwalkInfoReply::decode(&*body).unwrap().info.unwrap();
    assert_eq!(info.issue_id, Some(59));
    assert_eq!(
        info.end_time.unwrap() - info.time.unwrap(),
        30 * 24 * 60 * 60
    );
    let map = info.map_info.first().unwrap();
    let element = map.element_infos.first().unwrap();
    let element_id = element.element_id.unwrap();
    assert_eq!(element.visible, Some(true));
    assert_eq!(map.element_infos.len(), 1);
    assert_eq!(map.battle_infos.len(), 2);
    let expected_battle = map
        .battle_infos
        .iter()
        .find(|battle| battle.element_id == Some(element_id))
        .unwrap()
        .battle_id
        .unwrap();

    let mut data = Vec::new();
    BeforeStartWeekwalkBattleRequest {
        element_id: Some(element_id),
        layer_id: map.id,
    }
    .encode(&mut data)
    .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 2,
            cmd_id: CmdId::BeforeStartWeekwalkBattleCmd as i16,
            up_tag: 52,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();
    let CommandPacket::Reply {
        cmd_id: CmdId::BeforeStartWeekwalkBattleCmd,
        body,
        result_code: 0,
        up_tag: 52,
        ..
    } = packets.recv().await.unwrap()
    else {
        panic!("Weekwalk prebattle selection did not reach its handler");
    };
    let reply = BeforeStartWeekwalkBattleReply::decode(&*body).unwrap();
    assert_eq!(reply.element_id, Some(element_id));
    assert_eq!(reply.layer_id, map.id);
    assert_eq!(
        ctx.player().unwrap().weekwalk.selected_battle().unwrap(),
        logic::weekwalk::WeekwalkBattleContext {
            map_id: map.id.unwrap(),
            element_id,
            battle_id: expected_battle,
        }
    );

    let mut data = Vec::new();
    StartDungeonRequest {
        chapter_id: Some(901),
        episode_id: Some(90101),
        fight_group: Some(sonettobuf::FightGroup {
            hero_list,
            ..Default::default()
        }),
        multiplication: Some(1),
        ..Default::default()
    }
    .encode(&mut data)
    .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 3,
            cmd_id: CmdId::StartDungeonCmd as i16,
            up_tag: 53,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();
    let CommandPacket::Reply {
        cmd_id: CmdId::StartDungeonCmd,
        body,
        result_code: 0,
        up_tag: 53,
        ..
    } = packets.recv().await.unwrap()
    else {
        panic!("Weekwalk start did not reach the dungeon handler");
    };
    assert!(StartDungeonReply::decode(&*body).unwrap().fight.is_some());
    let CommandPacket::Push {
        cmd_id: CmdId::CardInfoPushCmd,
        ..
    } = packets.recv().await.unwrap()
    else {
        panic!("Weekwalk start did not project its cards");
    };
    let active = ctx.player().unwrap().battle.active_snapshot().unwrap();
    assert_eq!(active.battle_id, expected_battle);
    assert_eq!(
        active.weekwalk_context,
        Some(logic::weekwalk::WeekwalkBattleContext {
            map_id: map.id.unwrap(),
            element_id,
            battle_id: expected_battle,
        })
    );
    assert!(ctx.player().unwrap().weekwalk.selected_battle().is_err());

    let mut active = ctx.player().unwrap().battle.active_snapshot().unwrap();
    active.runtime = victory_runtime(expected_battle);
    ctx.player_mut().unwrap().battle.restore_active(active);
    let mut data = Vec::new();
    EndFightRequest {
        is_abort: Some(false),
    }
    .encode(&mut data)
    .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 4,
            cmd_id: CmdId::FightEndFightCmd as i16,
            up_tag: 54,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();
    let CommandPacket::Push {
        cmd_id: CmdId::WeekwalkInfoUpdatePushCmd,
        body,
        ..
    } = packets.recv().await.unwrap()
    else {
        panic!("Weekwalk victory did not project its progression");
    };
    let update = WeekwalkInfoUpdatePush::decode(&*body)
        .unwrap()
        .info
        .unwrap();
    let updated_map = update.map_info.first().unwrap();
    assert_eq!(updated_map.element_infos.len(), 2);
    assert!(updated_map.element_infos.iter().any(|element| {
        element.element_id == Some(element_id) && element.is_finish == Some(true)
    }));
    assert!(updated_map.element_infos.iter().any(|element| {
        element.element_id != Some(element_id) && element.visible == Some(true)
    }));
    let CommandPacket::Push {
        cmd_id: CmdId::FightEndFightPushCmd,
        ..
    } = packets.recv().await.unwrap()
    else {
        panic!("Weekwalk victory did not end its fight");
    };
    let CommandPacket::Reply {
        cmd_id: CmdId::FightEndFightCmd,
        body,
        result_code: 0,
        up_tag: 54,
        ..
    } = packets.recv().await.unwrap()
    else {
        panic!("Weekwalk victory did not reply");
    };
    EndFightReply::decode(&*body).unwrap();
    assert!(packets.try_recv().is_err());
    assert!(!ctx.player().unwrap().battle.has_active());
}

#[tokio::test]
async fn dungeon_end_victory_advances_weekwalk_without_dungeon_settlement() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 921;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'weekwalk-dungeon-end', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(4);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    let info = ctx
        .player()
        .unwrap()
        .weekwalk
        .info(ctx.state.db)
        .await
        .unwrap()
        .info
        .unwrap();
    let map = info.map_info.first().unwrap();
    let element_id = map.element_infos.first().unwrap().element_id.unwrap();
    let map_id = map.id.unwrap();
    let db = ctx.state.db;
    ctx.player_mut()
        .unwrap()
        .weekwalk
        .select_battle(db, map_id, element_id)
        .await
        .unwrap();
    let context = ctx.player().unwrap().weekwalk.selected_battle().unwrap();
    let fight_id = database::db::game::battle::create_fight_instance(
        ctx.state.db,
        database::db::game::battle::NewFightInstance {
            user_id: player_id,
            episode_id: 90101,
            battle_id: context.battle_id,
            multiplication: 1,
            entry_cost: "{}",
            checkpoint: "{}",
            created_at: 0,
        },
    )
    .await
    .unwrap();
    ctx.player_mut()
        .unwrap()
        .battle
        .restore_active(crate::player::battle::ActiveBattle {
            chapter_id: 901,
            episode_id: 90101,
            battle_id: context.battle_id,
            fight_id: Some(fight_id),
            runtime: victory_runtime(context.battle_id),
            weekwalk_context: Some(context),
            ..Default::default()
        });

    let mut data = Vec::new();
    EndDungeonRequest {
        is_abort: Some(false),
        ..Default::default()
    }
    .encode(&mut data)
    .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::DungeonEndDungeonCmd as i16,
            up_tag: 61,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();
    let CommandPacket::Push {
        cmd_id: CmdId::WeekwalkInfoUpdatePushCmd,
        body,
        ..
    } = packets.recv().await.unwrap()
    else {
        panic!("DungeonEnd did not project Weekwalk progression");
    };
    let update = WeekwalkInfoUpdatePush::decode(&*body)
        .unwrap()
        .info
        .unwrap();
    assert_eq!(update.map_info[0].element_infos.len(), 2);
    let CommandPacket::Reply {
        cmd_id: CmdId::DungeonEndDungeonCmd,
        body,
        result_code: 0,
        up_tag: 61,
        ..
    } = packets.recv().await.unwrap()
    else {
        panic!("DungeonEnd did not reply");
    };
    EndDungeonReply::decode(&*body).unwrap();
    assert!(packets.try_recv().is_err());
    assert!(!ctx.player().unwrap().battle.has_active());
    assert!(
        database::db::game::battle::load_active_fight(ctx.state.db, player_id)
            .await
            .unwrap()
            .is_none()
    );
    let dungeon_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM user_dungeons WHERE user_id = ?")
            .bind(player_id)
            .fetch_one(ctx.state.db)
            .await
            .unwrap();
    assert_eq!(dungeon_rows, 0);
}

#[tokio::test]
async fn ordinary_zero_battle_episode_stays_battleless() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 922;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'battleless-sentinel', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    let mut episode = None;
    for candidate in configs::get().episode.iter().filter(|episode| {
        episode.r#type != 9
            && episode.battle_id == 0
            && episode.first_battle_id == 0
            && episode.cost.is_empty()
            && episode.first_bonus == 0
            && episode.bonus == 0
            && episode.reward == 0
    }) {
        if crate::dungeon::can_start_episode(&pool, player_id, candidate.chapter_id, candidate.id)
            .await
            .unwrap()
        {
            episode = Some(candidate);
            break;
        }
    }
    let episode = episode.expect("config must contain an ordinary battleless episode");
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(64);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    let mut data = Vec::new();
    StartDungeonRequest {
        chapter_id: Some(episode.chapter_id),
        episode_id: Some(episode.id),
        multiplication: Some(1),
        ..Default::default()
    }
    .encode(&mut data)
    .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::StartDungeonCmd as i16,
            up_tag: 71,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();

    let mut reply = None;
    while let Ok(packet) = packets.try_recv() {
        if let CommandPacket::Reply {
            cmd_id: CmdId::StartDungeonCmd,
            body,
            result_code: 0,
            up_tag: 71,
            ..
        } = packet
        {
            reply = Some(StartDungeonReply::decode(&*body).unwrap());
        }
    }
    let reply = reply.expect("battleless start must reply");
    assert!(reply.fight.is_none());
    assert!(reply.round.is_none());
    assert!(!ctx.player().unwrap().battle.has_active());
}
