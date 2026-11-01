use super::*;
use sonettobuf::{FightGroup, StartDungeonReply, StartDungeonRequest};

#[tokio::test]
async fn reported_four_hero_team_starts_via_the_server_router() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    config::init(data_dir.to_str().unwrap()).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let user_id = 990001_i64;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
        .bind(user_id).bind("isolated-four-hero-team").execute(&pool).await.unwrap();
    let model = database::models::game::heros::UserHeroModel::new(user_id, pool.clone());
    let mut uids = Vec::new();
    for hero_id in [3120, 3088, 3126, 3125] {
        uids.push(model.create_hero(hero_id).await.unwrap());
    }
    // Synthetic progression and currency exist only in this in-memory database.
    sqlx::query("INSERT INTO user_dungeons (user_id, chapter_id, episode_id, star, challenge_count, \
        has_record, left_return_all_num, today_pass_num, today_total_num, created_at, updated_at) \
        VALUES (?, 104, 10421, 1, 0, 0, 1, 0, 0, 0, 0)")
        .bind(user_id).execute(&pool).await.unwrap();
    database::db::game::stories::finish_story(&pool, user_id, 100437).await.unwrap();
    sqlx::query("INSERT INTO currencies (user_id, currency_id, quantity) VALUES (?, 4, 100)")
        .bind(user_id).execute(&pool).await.unwrap();
    assert!(crate::dungeon::can_start_episode(&pool, user_id, 204, 20411).await.unwrap());

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(32);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(user_id, PlayerState::new(user_id, 0)));
    let request = StartDungeonRequest {
        chapter_id: Some(204), episode_id: Some(20411), multiplication: Some(1),
        fight_group: Some(FightGroup { hero_list: uids, cloth_id: Some(7), ..Default::default() }),
        ..Default::default()
    };
    let packet = ClientPacket { sequence: 1, cmd_id: CmdId::StartDungeonCmd as i16,
        up_tag: 1, data: request.encode_to_vec() }.encode();
    dispatch_command(&mut ctx, packet).await.unwrap();
    let mut reply = None;
    while let Ok(packet) = packets.try_recv() {
        if let CommandPacket::Reply { cmd_id: CmdId::StartDungeonCmd, body, result_code, .. } = packet {
            assert_eq!(result_code, 0, "server rejected four-hero battle request");
            reply = Some(StartDungeonReply::decode(&*body).unwrap());
        }
    }
    let reply = reply.expect("missing battle start reply");
    let fight = reply.fight.expect("battle data missing from reply");
    let models = fight.attacker.unwrap().entitys.into_iter().filter_map(|hero| hero.model_id).collect::<Vec<_>>();
    for hero_id in [3120, 3088, 3126, 3125] { assert!(models.contains(&hero_id), "missing hero {hero_id}"); }
    assert!(reply.round.is_some());
    assert!(ctx.player().unwrap().battle.has_active());
    let active: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM fight_instances WHERE user_id = ? AND active = 1")
        .bind(user_id).fetch_one(state.db).await.unwrap();
    assert_eq!(active, 1);
}

#[tokio::test]
async fn tides_of_thought_owned_support_starts_with_empty_assist_slot() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().join("data/excel2json");
    config::init(data_dir.to_str().unwrap()).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
        .connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let user_id = 990002_i64;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
        .bind(user_id).bind("isolated-tides-team").execute(&pool).await.unwrap();
    let model = database::models::game::heros::UserHeroModel::new(user_id, pool.clone());
    let mut fighter_uids = Vec::new();
    for hero_id in [3120, 3088, 3126, 3125] {
        fighter_uids.push(model.create_hero(hero_id).await.unwrap());
    }
    model.create_hero(3066).await.unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(32);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(user_id, PlayerState::new(user_id, 0)));
    let request = StartDungeonRequest {
        chapter_id: Some(9040), episode_id: Some(90400101), multiplication: Some(1),
        params: Some(r#"{"themeId":1,"layerId":1,"planeId":0,"supportId":306600,"researchId":0,"supportAssistUid":0,"supportAssistType":0}"#.into()),
        fight_group: Some(FightGroup {
            hero_list: fighter_uids.clone(), cloth_id: Some(7),
            params: Some(r#"{"heroList":["0","0","0","0"],"assistDataMap":[{"0":{}}],"buffParamsStr":"306600#0#0"}"#.into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let packet = ClientPacket { sequence: 1, cmd_id: CmdId::StartDungeonCmd as i16,
        up_tag: 1, data: request.encode_to_vec() }.encode();
    dispatch_command(&mut ctx, packet).await.unwrap();
    let mut reply = None;
    while let Ok(packet) = packets.try_recv() {
        if let CommandPacket::Reply { cmd_id: CmdId::StartDungeonCmd, body, result_code, .. } = packet {
            assert_eq!(result_code, 0);
            reply = Some(StartDungeonReply::decode(&*body).unwrap());
        }
    }
    let reply = reply.expect("missing Tides of Thought battle reply");
    let attacker = reply.fight.expect("battle data missing").attacker.unwrap();
    for uid in fighter_uids {
        assert!(attacker.entitys.iter().any(|hero| hero.uid == Some(uid)));
    }
    assert_eq!(attacker.assist_boss.unwrap().model_id, Some(306600));
    assert!(reply.round.is_some());
    assert!(ctx.player().unwrap().battle.has_active());
    let active: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM fight_instances WHERE user_id = ? AND active = 1")
        .bind(user_id).fetch_one(state.db).await.unwrap();
    assert_eq!(active, 1);
}

#[tokio::test]
async fn tides_boss_first_plane_starts_with_switch_metadata() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    config::init(data_dir.to_str().unwrap()).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let user_id = 990003_i64;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
        .bind(user_id).bind("isolated-tides-boss").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_tower_compose_theme_state (user_id, theme_id, pass_max_layer_id, updated_at) VALUES (?, 1, 5, 0)")
        .bind(user_id).execute(&pool).await.unwrap();
    let model = database::models::game::heros::UserHeroModel::new(user_id, pool.clone());
    let mut uids = Vec::new();
    for hero_id in [3120, 3088, 3126, 3125] { uids.push(model.create_hero(hero_id).await.unwrap()); }
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(32);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(user_id, PlayerState::new(user_id, 0)));
    let request = StartDungeonRequest {
        chapter_id: Some(9040), episode_id: Some(90400199), multiplication: Some(1),
        params: Some(r#"{"themeId":1,"layerId":6,"planeId":1,"supportId":0,"researchId":0,"supportAssistUid":0,"supportAssistType":0}"#.into()),
        fight_group: Some(FightGroup { hero_list: uids, cloth_id: Some(7), ..Default::default() }),
        ..Default::default()
    };
    let packet = ClientPacket { sequence: 1, cmd_id: CmdId::StartDungeonCmd as i16,
        up_tag: 1, data: request.encode_to_vec() }.encode();
    dispatch_command(&mut ctx, packet).await.unwrap();
    let mut reply = None;
    while let Ok(packet) = packets.try_recv() {
        if let CommandPacket::Reply { cmd_id: CmdId::StartDungeonCmd, body, result_code, .. } = packet {
            assert_eq!(result_code, 0);
            reply = Some(StartDungeonReply::decode(&*body).unwrap());
        }
    }
    let fight = reply.expect("missing boss start reply").fight.unwrap();
    let metadata = fight.custom_data.iter().find(|data| data.r#type == Some(8)).unwrap();
    let metadata: serde_json::Value = serde_json::from_str(metadata.data.as_deref().unwrap()).unwrap();
    assert_eq!(metadata["planeId"], 1);
    assert_eq!(metadata["maxPlaneId"], 2);
    assert!(ctx.player().unwrap().battle.has_active());
}

#[tokio::test]
async fn tides_two_plane_components_survive_client_roundtrip_and_boss_start() {
    use sonettobuf::{TowerComposeGetInfoReply, TowerComposeGetInfoRequest,
        TowerComposeMod, TowerComposeMods, TowerComposePlaneMods,
        TowerComposeSetModsReply, TowerComposeSetModsRequest};
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    config::init(data_dir.to_str().unwrap()).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
        .connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let user_id = 990004_i64;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
        .bind(user_id).bind("isolated-tides-components").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_tower_compose_theme_state (user_id, theme_id, pass_max_layer_id, updated_at) VALUES (?, 1, 5, 0)")
        .bind(user_id).execute(&pool).await.unwrap();
    let model = database::models::game::heros::UserHeroModel::new(user_id, pool.clone());
    let mut uids = Vec::new();
    for hero_id in [3041, 3034, 3127, 3066] { uids.push(model.create_hero(hero_id).await.unwrap()); }
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(32);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(user_id, PlayerState::new(user_id, 0)));
    let plane = |id: i32, body: &[i32], words: &[i32], env: i32| TowerComposePlaneMods {
        plane_id: Some(id),
        mods: [(1, body), (2, words), (3, std::slice::from_ref(&env))].into_iter()
            .map(|(kind, ids)| TowerComposeMods { r#type: Some(kind), mods: ids.iter().enumerate()
                .map(|(index, &mod_id)| TowerComposeMod { slot: Some(index as i32 + 1), mod_id: Some(mod_id) })
                .collect() }).collect(),
    };
    let first = plane(1, &[11101, 11201, 11301, 11415], &[11501, 11502, 11503, 11504, 11505], 11601);
    let second = plane(2, &[11102, 11202, 11302, 11412], &[11506, 11507, 11508, 11509, 0], 11602);
    // The first boss only sends plane 1. Its response must still supply a
    // usable default for the previously unsaved second plane.
    let one_plane = TowerComposeSetModsRequest { theme_id: Some(1), plane_mods: vec![first.clone()] };
    let packet = ClientPacket { sequence: 1, cmd_id: CmdId::TowerComposeSetModsCmd as i16,
        up_tag: 1, data: one_plane.encode_to_vec() }.encode();
    dispatch_command(&mut ctx, packet).await.unwrap();
    let response = packets.try_recv().unwrap();
    let CommandPacket::Reply { body, result_code, .. } = response else { panic!("expected set mods reply") };
    assert_eq!(result_code, 0);
    let response = TowerComposeSetModsReply::decode(&*body).unwrap();
    let default_second = response.planes.iter().find(|p| p.plane_id == Some(2)).unwrap();
    assert!(default_second.mods.iter().any(|group| group.r#type == Some(3)
        && group.mods.iter().any(|item| item.slot == Some(1) && item.mod_id == Some(11601))),
        "an unsaved second plane lost its default environment");
    let set = TowerComposeSetModsRequest { theme_id: Some(1), plane_mods: vec![first.clone(), second.clone()] };
    let packet = ClientPacket { sequence: 1, cmd_id: CmdId::TowerComposeSetModsCmd as i16,
        up_tag: 1, data: set.encode_to_vec() }.encode();
    dispatch_command(&mut ctx, packet).await.unwrap();
    let mut saved = None;
    while let Ok(packet) = packets.try_recv() {
        if let CommandPacket::Reply { cmd_id: CmdId::TowerComposeSetModsCmd, body, result_code, .. } = packet {
            assert_eq!(result_code, 0);
            saved = Some(TowerComposeSetModsReply::decode(&*body).unwrap());
        }
    }
    let saved = saved.expect("missing set mods reply");
    for expected in [&first, &second] {
        let got = saved.planes.iter().find(|p| p.plane_id == expected.plane_id).unwrap();
        assert_eq!(got.mods, expected.mods, "selection was lost in set mods reply");
    }
    // Revisiting the single-plane boss sends only plane 1; it must not
    // erase the second-plane choices previously saved for the later boss.
    let packet = ClientPacket { sequence: 2, cmd_id: CmdId::TowerComposeSetModsCmd as i16,
        up_tag: 2, data: one_plane.encode_to_vec() }.encode();
    dispatch_command(&mut ctx, packet).await.unwrap();
    let response = packets.try_recv().unwrap();
    let CommandPacket::Reply { body, result_code, .. } = response else { panic!("expected set mods reply") };
    assert_eq!(result_code, 0);
    let response = TowerComposeSetModsReply::decode(&*body).unwrap();
    assert_eq!(response.planes.iter().find(|p| p.plane_id == Some(2)).unwrap().mods,
        second.mods, "a first-plane-only update erased the second plane");
    let packet = ClientPacket { sequence: 2, cmd_id: CmdId::TowerComposeGetInfoCmd as i16,
        up_tag: 2, data: TowerComposeGetInfoRequest::default().encode_to_vec() }.encode();
    dispatch_command(&mut ctx, packet).await.unwrap();
    let mut info = None;
    while let Ok(packet) = packets.try_recv() {
        if let CommandPacket::Reply { cmd_id: CmdId::TowerComposeGetInfoCmd, body, result_code, .. } = packet {
            assert_eq!(result_code, 0);
            info = Some(TowerComposeGetInfoReply::decode(&*body).unwrap());
        }
    }
    let info = info.expect("missing info reply").info.unwrap();
    let boss = info.themes.iter().find(|theme| theme.theme_id == Some(1)).unwrap().boss.as_ref().unwrap();
    for expected in [&first, &second] {
        assert_eq!(boss.planes.iter().find(|p| p.plane_id == expected.plane_id).unwrap().mods,
            expected.mods, "selection was lost when loading the theme");
    }
    let request = StartDungeonRequest {
        chapter_id: Some(9040), episode_id: Some(90400199), multiplication: Some(1),
        params: Some(r#"{"themeId":1,"layerId":6,"planeId":1,"supportId":0,"researchId":1400,"supportAssistUid":0,"supportAssistType":0}"#.into()),
        fight_group: Some(FightGroup { hero_list: uids, cloth_id: Some(7), ..Default::default() }),
        ..Default::default()
    };
    let request = ClientPacket { sequence: 3, cmd_id: CmdId::StartDungeonCmd as i16,
        up_tag: 3, data: request.encode_to_vec() };
    let result = crate::handlers::dungeon::on_start_dungeon(&mut ctx, request).await;
    assert!(result.is_ok(), "boss start failed: {:?}", result.err());
    let mut started = false;
    while let Ok(packet) = packets.try_recv() {
        if let CommandPacket::Reply { cmd_id: CmdId::StartDungeonCmd, body, result_code, .. } = packet {
            assert_eq!(result_code, 0);
            let reply = StartDungeonReply::decode(&*body).unwrap();
            assert!(reply.fight.is_some() && reply.round.is_some());
            started = true;
        }
    }
    assert!(started, "missing boss start reply");
}
