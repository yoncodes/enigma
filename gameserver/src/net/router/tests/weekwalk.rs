use super::*;
use sonettobuf::{
    ChangeWeekwalkHeroGroupSelectRequest, GetWeekwalkInfoReply, GetWeekwalkInfoRequest,
    WeekwalkDialogHistoryReply, WeekwalkDialogHistoryRequest,
    WeekwalkVer2ChangeHeroGroupSelectRequest, WeekwalkVer2ChooseSkillRequest, StartDungeonRequest,
};

fn request<M: Message>(command: CmdId, message: M, tag: u8) -> Vec<u8> {
    ClientPacket { sequence: 1, cmd_id: command as i16, up_tag: tag, data: message.encode_to_vec() }.encode()
}

#[tokio::test]
async fn weekwalk_initial_info_history_and_team_commands_reach_the_real_router() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    config::init(data_dir.to_str().unwrap()).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let user_id = 965001_i64;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, 'weekwalk-isolated-router', 0, 0)")
        .bind(user_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_open_infos (user_id, open_id, is_open) VALUES (?, 36, 1)")
        .bind(user_id).execute(&pool).await.unwrap();
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(16);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(user_id, PlayerState::new(user_id, 0)));

    dispatch_command(&mut ctx, request(CmdId::GetWeekwalkInfoCmd, GetWeekwalkInfoRequest {}, 1)).await.unwrap();
    let CommandPacket::Reply { cmd_id: CmdId::GetWeekwalkInfoCmd, body, result_code: 0, .. } = packets.try_recv().unwrap()
        else { panic!("missing opening info") };
    let info = GetWeekwalkInfoReply::decode(&*body).unwrap().info.unwrap();
    assert_eq!(info.map_info[0].id, Some(101));
    assert_eq!(info.map_info[0].element_infos[0].element_id, Some(10111));

    dispatch_command(&mut ctx, request(CmdId::WeekwalkDialogHistoryCmd, WeekwalkDialogHistoryRequest {
        layer_id: Some(101), element_id: Some(10111), historylist: vec!["0#7".into()],
    }, 2)).await.unwrap();
    let CommandPacket::Reply { cmd_id: CmdId::WeekwalkDialogHistoryCmd, body, result_code: 0, .. } = packets.try_recv().unwrap()
        else { panic!("missing history reply") };
    assert_eq!(WeekwalkDialogHistoryReply::decode(&*body).unwrap().historylist, ["0#7"]);
    assert_eq!(database::db::game::weekwalk::get_weekwalk_info(ctx.state.db, user_id).await.unwrap().1[0]
        .element_infos[0].historylist, ["0#7"]);

    sqlx::query("INSERT INTO user_weekwalk_battles (user_id, map_id, battle_id) VALUES (?, 101, 800101)")
        .bind(user_id).execute(ctx.state.db).await.unwrap();
    dispatch_command(&mut ctx, request(CmdId::ChangeWeekwalkHeroGroupSelectCmd, ChangeWeekwalkHeroGroupSelectRequest {
        layer_id: Some(101), battle_id: Some(800101), select: Some(2),
    }, 3)).await.unwrap();
    let CommandPacket::Reply { cmd_id: CmdId::ChangeWeekwalkHeroGroupSelectCmd, result_code: 0, .. } = packets.try_recv().unwrap()
        else { panic!("missing team reply") };
    let selected: i32 = sqlx::query_scalar("SELECT hero_group_select FROM user_weekwalk_battles WHERE user_id = ?")
        .bind(user_id).fetch_one(ctx.state.db).await.unwrap();
    assert_eq!(selected, 2);

    sqlx::query("INSERT INTO user_weekwalk_v2_battles (user_id, layer_id, battle_id) VALUES (?, 1001, 830101)")
        .bind(user_id).execute(ctx.state.db).await.unwrap();
    dispatch_command(&mut ctx, request(CmdId::WeekwalkVer2ChangeHeroGroupSelectCmd, WeekwalkVer2ChangeHeroGroupSelectRequest {
        layer_id: Some(1001), battle_id: Some(830101), select: Some(3),
    }, 4)).await.unwrap();
    let CommandPacket::Reply { cmd_id: CmdId::WeekwalkVer2ChangeHeroGroupSelectCmd, result_code: 0, .. } = packets.try_recv().unwrap()
        else { panic!("missing v2 team reply") };
    let selected: i32 = sqlx::query_scalar("SELECT hero_group_select FROM user_weekwalk_v2_battles WHERE user_id = ?")
        .bind(user_id).fetch_one(ctx.state.db).await.unwrap();
    assert_eq!(selected, 3);
    sqlx::query("INSERT INTO user_weekwalk_v2_info (user_id, time_id) VALUES (?, 1)")
        .bind(user_id).execute(ctx.state.db).await.unwrap();
    dispatch_command(&mut ctx, request(CmdId::WeekwalkVer2ChooseSkillCmd, WeekwalkVer2ChooseSkillRequest {
        no: Some(3), skill_ids: vec![2004],
    }, 5)).await.unwrap();
    let CommandPacket::Reply { cmd_id: CmdId::WeekwalkVer2ChooseSkillCmd, result_code: 0, .. } = packets.try_recv().unwrap()
        else { panic!("missing v2 skill reply") };
    let skills: Vec<i32> = sqlx::query_scalar("SELECT skill_id FROM user_weekwalk_v2_snapshot_skills WHERE user_id = ? AND snapshot_no = 3")
        .bind(user_id).fetch_all(ctx.state.db).await.unwrap();
    assert_eq!(skills, [2004]);
    assert!(packets.try_recv().is_err());
}

#[tokio::test]
async fn unfinished_weekwalk_battles_are_not_reported_as_battleless_victories() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    config::init(data_dir.to_str().unwrap()).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let user_id = 965002_i64;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, 'weekwalk-battle-guard', 0, 0)")
        .bind(user_id).execute(&pool).await.unwrap();
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(16);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(user_id, PlayerState::new(user_id, 0)));
    for (episode_id, chapter_id) in [(90101, 901), (2600101, 26001)] {
        dispatch_command(&mut ctx, request(CmdId::StartDungeonCmd, StartDungeonRequest {
            episode_id: Some(episode_id), chapter_id: Some(chapter_id), ..Default::default()
        }, 6)).await.unwrap();
        let CommandPacket::Push { cmd_id: CmdId::ServerErrorInfoPushCmd, .. } = packets.try_recv().unwrap()
            else { panic!("missing error notification") };
        let CommandPacket::Reply { cmd_id: CmdId::StartDungeonCmd, result_code, .. } = packets.try_recv().unwrap()
            else { panic!("missing rejected start") };
        assert_ne!(result_code, 0);
    }
    let finished: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_dungeons WHERE user_id = ?")
        .bind(user_id).fetch_one(ctx.state.db).await.unwrap();
    assert_eq!(finished, 0);
    let fights: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM fight_instances WHERE user_id = ?")
        .bind(user_id).fetch_one(ctx.state.db).await.unwrap();
    assert_eq!(fights, 0);
}

#[tokio::test]
async fn opening_dialog_starts_first_configured_battle_and_only_victory_advances_it() {
    use sonettobuf::{BeforeStartWeekwalkBattleRequest, EndFightRequest, Fight, FightEntityInfo, FightGroup,
        FightTeam, HeroAttribute, StartDungeonReply, WeekwalkDialogRequest, WeekwalkInfoUpdatePush};
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    config::init(data_dir.to_str().unwrap()).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let user_id = 965003_i64;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, 'weekwalk-first-battle', 0, 0)")
        .bind(user_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_open_infos (user_id, open_id, is_open) VALUES (?, 36, 1)")
        .bind(user_id).execute(&pool).await.unwrap();
    database::db::game::weekwalk::ensure_initial_shallow_map(&pool, user_id).await.unwrap();
    let hero_model = database::models::game::heros::UserHeroModel::new(user_id, pool.clone());
    let mut heroes = Vec::new();
    for hero_id in [3120, 3088, 3126] { heroes.push(hero_model.create_hero(hero_id).await.unwrap()); }
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(64);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(user_id, PlayerState::new(user_id, 0)));

    for (element_id, option) in [(10111, 1), (10112, 0)] {
        dispatch_command(&mut ctx, request(CmdId::WeekwalkDialogCmd, WeekwalkDialogRequest {
            element_id: Some(element_id), layer_id: Some(101), option: Some(option),
        }, 1)).await.unwrap();
        assert!(matches!(packets.try_recv().unwrap(), CommandPacket::Reply { cmd_id: CmdId::WeekwalkDialogCmd, result_code: 0, .. }));
        let CommandPacket::Push { cmd_id: CmdId::WeekwalkInfoUpdatePushCmd, body, .. } = packets.try_recv().unwrap()
            else { panic!("missing updated map after dialog") };
        let update = WeekwalkInfoUpdatePush::decode(&*body).unwrap().info.unwrap();
        assert_eq!(update.map_info[0].element_infos[1].element_id, Some(10112));
    }
    dispatch_command(&mut ctx, request(CmdId::BeforeStartWeekwalkBattleCmd, BeforeStartWeekwalkBattleRequest {
        element_id: Some(10112), layer_id: Some(101),
    }, 2)).await.unwrap();
    assert!(matches!(packets.try_recv().unwrap(), CommandPacket::Reply { cmd_id: CmdId::BeforeStartWeekwalkBattleCmd, result_code: 0, .. }));

    let start = StartDungeonRequest { chapter_id: Some(901), episode_id: Some(90101), multiplication: Some(1),
        fight_group: Some(FightGroup { hero_list: heroes, cloth_id: Some(7), ..Default::default() }), ..Default::default() };
    for victory in [false, true] {
        dispatch_command(&mut ctx, request(CmdId::StartDungeonCmd, start.clone(), 3)).await.unwrap();
        let CommandPacket::Reply { cmd_id: CmdId::StartDungeonCmd, body, result_code: 0, .. } = packets.try_recv().unwrap()
            else { panic!("first battle did not start") };
        let start_reply = StartDungeonReply::decode(&*body).unwrap();
        assert_eq!(start_reply.fight.unwrap().battle_id, Some(800101));
        assert!(matches!(packets.try_recv().unwrap(), CommandPacket::Push { cmd_id: CmdId::CardInfoPushCmd, .. }));
        if victory {
            let mut active = ctx.player().unwrap().battle.active_snapshot().unwrap();
            let entity = |uid, hp, side| FightEntityInfo { uid: Some(uid), current_hp: Some(hp), team_type: Some(side),
                attr: Some(HeroAttribute { hp: Some(100), ..Default::default() }), ..Default::default() };
            active.runtime = ::battle::tooling::unstarted_battle(::battle::catalog::BattleCatalog::new(configs::get()), Fight {
                episode_id: Some(90101), battle_id: Some(800101), cur_round: Some(1), cur_wave: Some(2),
                attacker: Some(FightTeam { entitys: vec![entity(10,100,1)], ..Default::default() }),
                defender: Some(FightTeam { entitys: vec![entity(-1,0,2)], ..Default::default() }), ..Default::default()
            });
            assert!(active.is_victory());
            ctx.player_mut().unwrap().battle.restore_active(active);
        }
        dispatch_command(&mut ctx, request(CmdId::FightEndFightCmd, EndFightRequest { is_abort: Some(!victory) }, 4)).await.unwrap();
        let mut saw_info = false;
        let mut saw_end = false;
        let mut saw_reply = false;
        while let Ok(packet) = packets.try_recv() {
            match packet {
                CommandPacket::Push { cmd_id: CmdId::WeekwalkInfoUpdatePushCmd, .. } => saw_info = true,
                CommandPacket::Push { cmd_id: CmdId::FightEndFightPushCmd, .. } => saw_end = true,
                CommandPacket::Reply { cmd_id: CmdId::FightEndFightCmd, result_code: 0, .. } => saw_reply = true,
                _ => {}
            }
        }
        assert!(saw_info && saw_end && saw_reply);
        let (_, maps) = database::db::game::weekwalk::get_weekwalk_info(ctx.state.db, user_id).await.unwrap();
        assert_eq!(maps[0].element_infos[1].index, Some(if victory { 2 } else { 1 }));
        assert_eq!(maps[0].battle_infos[0].star.unwrap() > 0, victory);
    }
}
