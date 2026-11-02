use super::*;
use sonettobuf::{
    ChangeMapReply, ChangeMapRequest, EndDungeonRequest, ExploreInteractRequest, ExploreMoveRequest, ExploreStepPush, FightGroup, GetExploreInfoReply,
    ExploreItemChangePush, ExploreItemInteractRequest, ExploreUseItemRequest, ResetExploreRequest,
    GetExploreInfoRequest, GetExploreSimpleInfoReply, GetExploreSimpleInfoRequest,
    StartDungeonReply, StartDungeonRequest,
};

fn request<M: Message>(cmd: CmdId, message: M, tag: u8) -> Vec<u8> {
    ClientPacket { sequence: 1, cmd_id: cmd as i16, up_tag: tag, data: message.encode_to_vec() }.encode()
}

#[tokio::test]
async fn portable_rock_and_pressure_plate_use_the_actual_router() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    config::init(data_dir.to_str().unwrap()).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let user_id = 950003_i64;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
        .bind(user_id).bind("isolated-rock-router").execute(&pool).await.unwrap();
    let layout = config::explore_layout::layout(102).unwrap().unwrap();
    let mut map = database::db::game::explore_runtime::MapState::new(layout);
    map.x = 5; map.y = 12;
    let mut tx = pool.begin().await.unwrap();
    database::db::game::explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
    database::db::game::explore_runtime::insert_map(&mut tx, user_id, &map).await.unwrap();
    database::db::game::explore_runtime::set_current_map(&mut tx, user_id, 102).await.unwrap();
    tx.commit().await.unwrap();
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(32);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(user_id, PlayerState::new(user_id, 0)));

    dispatch_command(&mut ctx, request(CmdId::ExploreInteractCmd, ExploreInteractRequest { id: Some(29), params: None }, 1)).await.unwrap();
    let CommandPacket::Push { cmd_id: CmdId::ExploreItemChangePushCmd, body, .. } = packets.try_recv().unwrap() else { panic!("missing rock grant") };
    let item = ExploreItemChangePush::decode(&*body).unwrap().explore_items[0];
    assert_eq!((item.item_id, item.quantity), (Some(150001), Some(1)));
    let uid = item.uid.unwrap();
    let CommandPacket::Push { cmd_id: CmdId::ExploreStepPushCmd, .. } = packets.try_recv().unwrap() else { panic!("missing table update") };
    let CommandPacket::Reply { cmd_id: CmdId::ExploreInteractCmd, result_code: 0, .. } = packets.try_recv().unwrap() else { panic!("missing table reply") };

    let mut tx = state.db.begin().await.unwrap();
    let mut stored = database::db::game::explore_runtime::load_map(&mut tx, user_id, 102).await.unwrap().unwrap();
    stored.state.x = 5; stored.state.y = 5;
    database::db::game::explore_runtime::save_map(&mut tx, user_id, &mut stored).await.unwrap();
    tx.commit().await.unwrap();
    dispatch_command(&mut ctx, request(CmdId::ExploreUseItemCmd,
        ExploreUseItemRequest { uid: Some(uid), posx: Some(6), posy: Some(5), interact_id: None }, 2)).await.unwrap();
    let CommandPacket::Push { cmd_id: CmdId::ExploreItemChangePushCmd, body, .. } = packets.try_recv().unwrap() else { panic!("missing placement inventory update") };
    assert_eq!(ExploreItemChangePush::decode(&*body).unwrap().explore_items[0].quantity, Some(0));
    let CommandPacket::Push { cmd_id: CmdId::ExploreStepPushCmd, body, .. } = packets.try_recv().unwrap() else { panic!("missing placement updates") };
    let steps = ExploreStepPush::decode(&*body).unwrap();
    assert_eq!(steps.steps.len(), 3); // rock, plate, and door
    let CommandPacket::Reply { cmd_id: CmdId::ExploreUseItemCmd, result_code: 0, .. } = packets.try_recv().unwrap() else { panic!("missing placement reply") };

    dispatch_command(&mut ctx, request(CmdId::ExploreItemInteractCmd,
        ExploreItemInteractRequest { id: Some(uid as i32), params: Some(String::new()) }, 3)).await.unwrap();
    let CommandPacket::Push { cmd_id: CmdId::ExploreItemChangePushCmd, body, .. } = packets.try_recv().unwrap() else { panic!("missing retrieved rock") };
    assert_eq!(ExploreItemChangePush::decode(&*body).unwrap().explore_items[0].quantity, Some(1));
    let CommandPacket::Push { cmd_id: CmdId::ExploreStepPushCmd, body, .. } = packets.try_recv().unwrap() else { panic!("missing pickup updates") };
    assert_eq!(ExploreStepPush::decode(&*body).unwrap().steps.len(), 3);
    let CommandPacket::Reply { cmd_id: CmdId::ExploreItemInteractCmd, result_code: 0, .. } = packets.try_recv().unwrap() else { panic!("missing pickup reply") };
    let mut tx = state.db.begin().await.unwrap();
    let stored = database::db::game::explore_runtime::load_map(&mut tx, user_id, 102).await.unwrap().unwrap();
    assert_eq!((stored.state.objects[&14].status & 4, stored.state.objects[&2].status & 4), (0, 0));
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn ice_reset_and_spike_return_use_the_actual_router() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    config::init(data_dir.to_str().unwrap()).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let user_id = 950004_i64;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
        .bind(user_id).bind("isolated-trap-router").execute(&pool).await.unwrap();
    let mut map = database::db::game::explore_runtime::MapState::new(config::explore_layout::layout(102).unwrap().unwrap());
    map.x = 14; map.y = 14;
    let mut tx = pool.begin().await.unwrap();
    database::db::game::explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
    database::db::game::explore_runtime::insert_map(&mut tx, user_id, &map).await.unwrap();
    database::db::game::explore_runtime::set_current_map(&mut tx, user_id, 102).await.unwrap();
    tx.commit().await.unwrap();
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(32);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(user_id, PlayerState::new(user_id, 0)));
    for (x, y) in [(15, 14), (16, 14)] {
        dispatch_command(&mut ctx, request(CmdId::ExploreMoveCmd,
            ExploreMoveRequest { posx: Some(x), posy: Some(y), interact_id: None }, 1)).await.unwrap();
        while packets.try_recv().is_ok() {}
    }
    dispatch_command(&mut ctx, request(CmdId::ResetExploreCmd, ResetExploreRequest {}, 2)).await.unwrap();
    let CommandPacket::Push { cmd_id: CmdId::ExploreStepPushCmd, body, .. } = packets.try_recv().unwrap() else { panic!("missing reset steps") };
    let steps = ExploreStepPush::decode(&*body).unwrap();
    let step: serde_json::Value = serde_json::from_str(steps.steps.last().unwrap().param.as_ref().unwrap()).unwrap();
    assert_eq!((step["stepType"].as_i64(), step["x"].as_i64(), step["y"].as_i64()), (Some(1), Some(15), Some(14)));
    let CommandPacket::Reply { cmd_id: CmdId::ResetExploreCmd, result_code: 0, .. } = packets.try_recv().unwrap() else { panic!("missing reset reply") };

    let mut tx = state.db.begin().await.unwrap();
    let mut stored = database::db::game::explore_runtime::load_map(&mut tx, user_id, 102).await.unwrap().unwrap();
    stored.state.x = 10; stored.state.y = 3;
    database::db::game::explore_runtime::save_map(&mut tx, user_id, &mut stored).await.unwrap();
    tx.commit().await.unwrap();
    dispatch_command(&mut ctx, request(CmdId::ExploreInteractCmd, ExploreInteractRequest { id: Some(20), params: None }, 3)).await.unwrap();
    let CommandPacket::Push { cmd_id: CmdId::ExploreStepPushCmd, body, .. } = packets.try_recv().unwrap() else { panic!("missing trap step") };
    let steps = ExploreStepPush::decode(&*body).unwrap();
    let trap: serde_json::Value = serde_json::from_str(steps.steps[steps.steps.len() - 2].param.as_ref().unwrap()).unwrap();
    assert_eq!((trap["stepType"].as_i64(), trap["triggerInteractId"].as_i64(), trap["x"].as_i64(), trap["y"].as_i64()),
        (Some(7), Some(20), Some(1), Some(2)));
    let CommandPacket::Reply { cmd_id: CmdId::ExploreInteractCmd, result_code: 0, .. } = packets.try_recv().unwrap() else { panic!("missing trap reply") };
}

#[tokio::test]
async fn first_light_prism_rotation_uses_the_actual_router() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    config::init(data_dir.to_str().unwrap()).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let user_id = 950005_i64;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
        .bind(user_id).bind("isolated-light-router").execute(&pool).await.unwrap();
    let mut map = database::db::game::explore_runtime::MapState::new(config::explore_layout::layout(301).unwrap().unwrap());
    map.x = -5; map.y = 4;
    let mut tx = pool.begin().await.unwrap();
    database::db::game::explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
    database::db::game::explore_runtime::insert_map(&mut tx, user_id, &map).await.unwrap();
    database::db::game::explore_runtime::set_current_map(&mut tx, user_id, 301).await.unwrap();
    tx.commit().await.unwrap();
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(32);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(user_id, PlayerState::new(user_id, 0)));
    for direction in [45, 0] {
        dispatch_command(&mut ctx, request(CmdId::ExploreInteractCmd,
            ExploreInteractRequest { id: Some(298), params: Some("27#-1".into()) }, 1)).await.unwrap();
        let CommandPacket::Push { cmd_id: CmdId::ExploreStepPushCmd, body, .. } = packets.try_recv().unwrap() else { panic!("missing prism rotation") };
        let steps = ExploreStepPush::decode(&*body).unwrap();
        let step: serde_json::Value = serde_json::from_str(steps.steps[0].param.as_ref().unwrap()).unwrap();
        assert_eq!((step["stepType"].as_i64(), step["interactId"].as_i64(), step["newDir"].as_i64()),
            (Some(13), Some(298), Some(direction)));
        let CommandPacket::Reply { cmd_id: CmdId::ExploreInteractCmd, result_code: 0, .. } = packets.try_recv().unwrap() else { panic!("missing rotation reply") };
    }
    let mut tx = state.db.begin().await.unwrap();
    let stored = database::db::game::explore_runtime::load_map(&mut tx, user_id, 301).await.unwrap().unwrap().state;
    assert_eq!((stored.objects[&136].status & 4, stored.objects[&297].status & 4, stored.objects[&118].status & 4), (4,4,4));
    tx.commit().await.unwrap();
    // Reposition the isolated test player next to a configured movable prism.
    let mut tx = state.db.begin().await.unwrap();
    let mut stored = database::db::game::explore_runtime::load_map(&mut tx, user_id, 301).await.unwrap().unwrap();
    stored.state.x = 0; stored.state.y = 21;
    database::db::game::explore_runtime::save_map(&mut tx, user_id, &mut stored).await.unwrap();
    tx.commit().await.unwrap();
    dispatch_command(&mut ctx, request(CmdId::ExploreMoveCmd,
        ExploreMoveRequest { posx: Some(2), posy: Some(21), interact_id: Some(176) }, 2)).await.unwrap();
    let CommandPacket::Push { cmd_id: CmdId::ExploreStepPushCmd, .. } = packets.try_recv().unwrap() else { panic!("missing movable prism updates") };
    let CommandPacket::Reply { cmd_id: CmdId::ExploreMoveCmd, result_code: 0, .. } = packets.try_recv().unwrap() else { panic!("missing movable prism reply") };
    let mut tx = state.db.begin().await.unwrap();
    let saved = database::db::game::explore_runtime::load_map(&mut tx, user_id, 301).await.unwrap().unwrap().state;
    assert_eq!((saved.x, saved.y, saved.objects[&176].x, saved.objects[&176].y), (1,21,2,21));
    tx.commit().await.unwrap();

    let mut tx = state.db.begin().await.unwrap();
    let mut saved = database::db::game::explore_runtime::load_map(&mut tx, user_id, 301).await.unwrap().unwrap();
    saved.state.x = 22; saved.state.y = 18;
    database::db::game::explore_runtime::save_map(&mut tx, user_id, &mut saved).await.unwrap();
    let repair_uid = database::db::game::explore_runtime::grant_item(&mut tx, user_id, 150004, 1).await.unwrap()[0].uid.unwrap();
    tx.commit().await.unwrap();
    dispatch_command(&mut ctx, request(CmdId::ExploreUseItemCmd,
        ExploreUseItemRequest { uid: Some(repair_uid), posx: Some(0), posy: Some(0), interact_id: Some(181) }, 3)).await.unwrap();
    let CommandPacket::Push { cmd_id: CmdId::ExploreItemChangePushCmd, body, .. } = packets.try_recv().unwrap() else { panic!("missing repair item update") };
    assert_eq!(ExploreItemChangePush::decode(&*body).unwrap().explore_items[0].quantity, Some(0));
    let CommandPacket::Push { cmd_id: CmdId::ExploreStepPushCmd, .. } = packets.try_recv().unwrap() else { panic!("missing repair steps") };
    let CommandPacket::Reply { cmd_id: CmdId::ExploreUseItemCmd, result_code: 0, .. } = packets.try_recv().unwrap() else { panic!("missing repair reply") };

    let mut tx = state.db.begin().await.unwrap();
    let mut saved = database::db::game::explore_runtime::load_map(&mut tx, user_id, 301).await.unwrap().unwrap();
    saved.state.x = -15; saved.state.y = 19;
    saved.state.objects.get_mut(&309).unwrap().direction = 90;
    database::db::game::explore_runtime::save_map(&mut tx, user_id, &mut saved).await.unwrap();
    let light_uid = database::db::game::explore_runtime::grant_item(&mut tx, user_id, 150003, 1).await.unwrap()[0].uid.unwrap();
    tx.commit().await.unwrap();
    for (x,y) in [(-15,18), (0,0)] {
        dispatch_command(&mut ctx, request(CmdId::ExploreUseItemCmd,
            ExploreUseItemRequest { uid: Some(light_uid), posx: Some(x), posy: Some(y), interact_id: None }, 4)).await.unwrap();
        let CommandPacket::Push { cmd_id: CmdId::ExploreStepPushCmd, .. } = packets.try_recv().unwrap() else { panic!("missing portable light steps") };
        let CommandPacket::Reply { cmd_id: CmdId::ExploreUseItemCmd, result_code: 0, .. } = packets.try_recv().unwrap() else { panic!("missing portable light reply") };
    }
}

#[tokio::test]
async fn first_pipe_junction_uses_the_actual_router() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    config::init(data_dir.to_str().unwrap()).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let user_id = 950006_i64;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
        .bind(user_id).bind("isolated-pipe-router").execute(&pool).await.unwrap();
    let mut map = database::db::game::explore_runtime::MapState::new(config::explore_layout::layout(201).unwrap().unwrap());
    map.x = 10; map.y = -11;
    let mut tx = pool.begin().await.unwrap();
    database::db::game::explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
    database::db::game::explore_runtime::insert_map(&mut tx, user_id, &map).await.unwrap();
    database::db::game::explore_runtime::set_current_map(&mut tx, user_id, 201).await.unwrap();
    tx.commit().await.unwrap();
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(32);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(user_id, PlayerState::new(user_id, 0)));
    dispatch_command(&mut ctx, request(CmdId::ExploreInteractCmd,
        ExploreInteractRequest { id: Some(14), params: Some("27#-1".into()) }, 1)).await.unwrap();
    let CommandPacket::Push { cmd_id: CmdId::ExploreStepPushCmd, body, .. } = packets.try_recv().unwrap() else { panic!("missing pipe rotation") };
    let steps = ExploreStepPush::decode(&*body).unwrap();
    let step: serde_json::Value = serde_json::from_str(steps.steps[0].param.as_ref().unwrap()).unwrap();
    assert_eq!((step["stepType"].as_i64(), step["interactId"].as_i64(), step["newDir"].as_i64()), (Some(13), Some(14), Some(0)));
    let CommandPacket::Reply { cmd_id: CmdId::ExploreInteractCmd, result_code: 0, .. } = packets.try_recv().unwrap() else { panic!("missing pipe reply") };
    let mut tx = state.db.begin().await.unwrap();
    let saved = database::db::game::explore_runtime::load_map(&mut tx, user_id, 201).await.unwrap().unwrap().state;
    assert_eq!((saved.objects[&15].status & 4, saved.objects[&16].status & 4, saved.objects[&18].status & 4), (4,4,4));
    tx.commit().await.unwrap();
    let mut tx = state.db.begin().await.unwrap();
    let mut saved = database::db::game::explore_runtime::load_map(&mut tx, user_id, 201).await.unwrap().unwrap();
    saved.state.x = 10; saved.state.y = -9;
    database::db::game::explore_runtime::save_map(&mut tx, user_id, &mut saved).await.unwrap();
    tx.commit().await.unwrap();
    for (id, params) in [(2, None), (17, Some("11#-9")), (17, None)] {
        dispatch_command(&mut ctx, request(CmdId::ExploreInteractCmd,
            ExploreInteractRequest { id: Some(id), params: params.map(str::to_owned) }, 2)).await.unwrap();
        let CommandPacket::Push { cmd_id: CmdId::ExploreStepPushCmd, .. } = packets.try_recv().unwrap() else { panic!("missing pipe pot steps") };
        let CommandPacket::Reply { cmd_id: CmdId::ExploreInteractCmd, result_code: 0, .. } = packets.try_recv().unwrap() else { panic!("missing pipe pot reply") };
    }
    dispatch_command(&mut ctx, request(CmdId::ExploreMoveCmd,
        ExploreMoveRequest { posx: Some(10), posy: Some(-9), interact_id: None }, 3)).await.unwrap();
    let CommandPacket::Push { cmd_id: CmdId::ExploreStepPushCmd, .. } = packets.try_recv().unwrap() else { panic!("missing carried movement") };
    let CommandPacket::Reply { cmd_id: CmdId::ExploreMoveCmd, result_code: 0, .. } = packets.try_recv().unwrap() else { panic!("missing carried movement reply") };
    dispatch_command(&mut ctx, request(CmdId::ExploreInteractCmd,
        ExploreInteractRequest { id: Some(2), params: None }, 4)).await.unwrap();
    let CommandPacket::Push { cmd_id: CmdId::ExploreStepPushCmd, .. } = packets.try_recv().unwrap() else { panic!("missing pipe rebind steps") };
    let CommandPacket::Reply { cmd_id: CmdId::ExploreInteractCmd, result_code: 0, .. } = packets.try_recv().unwrap() else { panic!("missing pipe rebind reply") };
    let mut tx = state.db.begin().await.unwrap();
    let saved = database::db::game::explore_runtime::load_map(&mut tx, user_id, 201).await.unwrap().unwrap().state;
    let entry: serde_json::Value = serde_json::from_str(&saved.objects[&2].status2).unwrap();
    assert_eq!(entry["bindInteractId"].as_i64(), Some(17));
    assert_eq!(database::db::game::explore_runtime::use_item_uid(&mut tx, user_id).await.unwrap(), Some(0));
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn directed_pipe_rotation_is_reported_and_saved_by_router() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().join("data/excel2json");
    config::init(data_dir.to_str().unwrap()).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
        .connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let user_id = 950010_i64;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
        .bind(user_id).bind("isolated-directed-pipe").execute(&pool).await.unwrap();
    let layout = config::explore_layout::layout(201).unwrap().unwrap();
    let mut map = database::db::game::explore_runtime::MapState::new(layout);
    map.x = 73; map.y = 0;
    let mut tx = pool.begin().await.unwrap();
    database::db::game::explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
    database::db::game::explore_runtime::insert_map(&mut tx, user_id, &map).await.unwrap();
    database::db::game::explore_runtime::set_current_map(&mut tx, user_id, 201).await.unwrap();
    tx.commit().await.unwrap();
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(32);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(user_id, PlayerState::new(user_id, 0)));
    dispatch_command(&mut ctx, request(CmdId::ExploreInteractCmd,
        ExploreInteractRequest { id: Some(196), params: Some("27#1".into()) }, 1)).await.unwrap();
    let CommandPacket::Push { cmd_id: CmdId::ExploreStepPushCmd, body, .. } = packets.try_recv().unwrap()
        else { panic!("missing directed pipe rotation") };
    let steps = ExploreStepPush::decode(&*body).unwrap();
    let step: serde_json::Value = serde_json::from_str(steps.steps[0].param.as_ref().unwrap()).unwrap();
    assert_eq!((step["interactId"].as_i64(), step["newDir"].as_i64()), (Some(196), Some(90)));
    assert!(matches!(packets.try_recv().unwrap(), CommandPacket::Reply {
        cmd_id: CmdId::ExploreInteractCmd, result_code: 0, ..
    }));
    let mut tx = state.db.begin().await.unwrap();
    let saved = database::db::game::explore_runtime::load_map(&mut tx, user_id, 201)
        .await.unwrap().unwrap().state;
    assert_eq!(saved.objects[&196].direction, 90);
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn bonus_scene_choices_are_saved_via_the_actual_router() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    config::init(data_dir.to_str().unwrap()).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let user_id = 950007_i64;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
        .bind(user_id).bind("isolated-bonus-router").execute(&pool).await.unwrap();
    let mut map = database::db::game::explore_runtime::MapState::new(config::explore_layout::layout(103).unwrap().unwrap());
    map.x = 41; map.y = 94;
    let mut tx = pool.begin().await.unwrap();
    database::db::game::explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
    database::db::game::explore_runtime::insert_map(&mut tx, user_id, &map).await.unwrap();
    database::db::game::explore_runtime::set_current_map(&mut tx, user_id, 103).await.unwrap();
    tx.commit().await.unwrap();
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(32);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(user_id, PlayerState::new(user_id, 0)));
    dispatch_command(&mut ctx, request(CmdId::ExploreInteractCmd,
        ExploreInteractRequest { id: Some(710), params: Some("-1#2#-1".into()) }, 1)).await.unwrap();
    let CommandPacket::Push { cmd_id: CmdId::ExploreStepPushCmd, body, .. } = packets.try_recv().unwrap() else { panic!("missing bonus scene updates") };
    let steps = ExploreStepPush::decode(&*body).unwrap();
    let scene: serde_json::Value = serde_json::from_str(steps.steps.last().unwrap().param.as_ref().unwrap()).unwrap();
    assert_eq!((scene["stepType"].as_i64(), scene["bonusSceneId"].as_i64()), (Some(17), Some(1401301)));
    assert_eq!(scene["options"], serde_json::json!([-1,2,-1]));
    let CommandPacket::Reply { cmd_id: CmdId::ExploreInteractCmd, result_code: 0, .. } = packets.try_recv().unwrap() else { panic!("missing bonus scene reply") };
    let (_, chapters, _, _) = database::db::game::explore::get_explore_info(state.db, user_id).await.unwrap();
    assert_eq!(chapters[0].bonus_scenes[0].options, vec![-1,2,-1]);
}

#[tokio::test]
async fn exploration_encounter_starts_and_aborts_via_the_actual_router() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    config::init(data_dir.to_str().unwrap()).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let user_id = 950002_i64;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, ?, 0, 0)")
        .bind(user_id).bind("isolated-explore-battle-route").execute(&pool).await.unwrap();
    let hero = database::models::game::heros::UserHeroModel::new(user_id, pool.clone())
        .create_hero(3088).await.unwrap();
    let layout = config::explore_layout::layout(101).unwrap().unwrap();
    let mut map = database::db::game::explore_runtime::MapState::new(layout);
    // Start next to a configured encounter to isolate the battle protocol from pathfinding.
    map.x = 29;
    map.y = 14;
    let mut tx = pool.begin().await.unwrap();
    database::db::game::explore_runtime::ensure_profile(&mut tx, user_id).await.unwrap();
    database::db::game::explore_runtime::insert_map(&mut tx, user_id, &map).await.unwrap();
    database::db::game::explore_runtime::set_current_map(&mut tx, user_id, 101).await.unwrap();
    tx.commit().await.unwrap();
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(32);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(user_id, PlayerState::new(user_id, 0)));

    dispatch_command(&mut ctx, request(CmdId::ExploreInteractCmd, ExploreInteractRequest { id: Some(431), params: None }, 1)).await.unwrap();
    let CommandPacket::Push { cmd_id: CmdId::ExploreStepPushCmd, body, .. } = packets.try_recv().unwrap() else { panic!("missing encounter push") };
    let steps = ExploreStepPush::decode(&*body).unwrap();
    let step: serde_json::Value = serde_json::from_str(steps.steps[0].param.as_ref().unwrap()).unwrap();
    assert_eq!((step["stepType"].as_i64(), step["battleId"].as_i64()), (Some(12), Some(501101)));
    let CommandPacket::Reply { cmd_id: CmdId::ExploreInteractCmd, result_code: 0, .. } = packets.try_recv().unwrap() else { panic!("missing interaction reply") };

    let start = StartDungeonRequest { chapter_id: Some(1401), episode_id: Some(140101),
        multiplication: Some(1), fight_group: Some(FightGroup { hero_list: vec![hero], cloth_id: Some(7), ..Default::default() }),
        ..Default::default() };
    dispatch_command(&mut ctx, request(CmdId::StartDungeonCmd, start, 2)).await.unwrap();
    let CommandPacket::Reply { cmd_id: CmdId::StartDungeonCmd, body, result_code: 0, .. } = packets.try_recv().unwrap() else { panic!("exploration battle did not start") };
    let start_reply = StartDungeonReply::decode(&*body).unwrap();
    assert_eq!(start_reply.fight.unwrap().battle_id, Some(501101));
    assert!(start_reply.round.is_some());
    assert!(ctx.player().unwrap().battle.has_active());
    while packets.try_recv().is_ok() {}

    dispatch_command(&mut ctx, request(CmdId::DungeonEndDungeonCmd, EndDungeonRequest { is_abort: Some(true), end_type: None }, 3)).await.unwrap();
    let mut ended = false;
    while let Ok(packet) = packets.try_recv() {
        if let CommandPacket::Reply { cmd_id: CmdId::DungeonEndDungeonCmd, result_code, .. } = packet {
            assert_eq!(result_code, 0);
            ended = true;
        }
    }
    assert!(ended && !ctx.player().unwrap().battle.has_active());
    assert!(logic::explore_battle::pending(state.db, user_id).await.unwrap().is_none());
    let mut tx = state.db.begin().await.unwrap();
    let restored = database::db::game::explore_runtime::load_map(&mut tx, user_id, 101).await.unwrap().unwrap().state;
    assert_eq!((restored.x, restored.y), (29, 14));
    assert_eq!(restored.objects[&431].status & 8, 0);
    tx.commit().await.unwrap();
    let completed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_dungeons WHERE user_id = ? AND episode_id = 140101")
        .bind(user_id).fetch_one(state.db).await.unwrap();
    assert_eq!(completed, 0);
}

#[tokio::test]
async fn exploration_entry_move_and_resume_use_actual_router() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    config::init(data_dir.to_str().unwrap()).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (950001, 'isolated-explore-route', 0, 0)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_open_infos (user_id, open_id, is_open) VALUES (950001, 47, 1)")
        .execute(&pool).await.unwrap();
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(32);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(950001, PlayerState::new(950001, 0)));

    dispatch_command(&mut ctx, request(CmdId::GetExploreSimpleInfoCmd, GetExploreSimpleInfoRequest {}, 1)).await.unwrap();
    let CommandPacket::Reply { body, result_code: 0, .. } = packets.try_recv().unwrap() else { panic!("missing exploration summary") };
    let simple = GetExploreSimpleInfoReply::decode(&*body).unwrap();
    assert_eq!(simple.unlock_map_ids, vec![101]);
    assert_eq!(simple.map_simple[0].purple_coin_total, Some(25));

    dispatch_command(&mut ctx, request(CmdId::ChangeMapCmd, ChangeMapRequest { map_id: Some(101) }, 2)).await.unwrap();
    let CommandPacket::Reply { body, result_code: 0, .. } = packets.try_recv().unwrap() else { panic!("missing map entry reply") };
    let entry = ChangeMapReply::decode(&*body).unwrap().explore_info.unwrap().explore_map.unwrap();
    assert_eq!((entry.map_id, entry.posx, entry.posy), (Some(101), Some(1), Some(6)));
    assert_eq!(entry.interacts.len(), 199);

    dispatch_command(&mut ctx, request(CmdId::ExploreMoveCmd, ExploreMoveRequest { posx: Some(1), posy: Some(5), interact_id: None }, 3)).await.unwrap();
    let CommandPacket::Push { cmd_id: CmdId::ExploreStepPushCmd, body, .. } = packets.try_recv().unwrap() else { panic!("missing movement position push") };
    let movement = ExploreStepPush::decode(&*body).unwrap();
    let position: serde_json::Value = serde_json::from_str(movement.steps[0].param.as_ref().unwrap()).unwrap();
    assert_eq!((position["stepType"].as_i64(), position["x"].as_i64(), position["y"].as_i64()), (Some(1), Some(1), Some(5)));
    let CommandPacket::Reply { cmd_id: CmdId::ExploreMoveCmd, result_code: 0, .. } = packets.try_recv().unwrap() else { panic!("missing movement reply") };
    dispatch_command(&mut ctx, request(CmdId::GetExploreInfoCmd, GetExploreInfoRequest {}, 4)).await.unwrap();
    let CommandPacket::Reply { body, result_code: 0, .. } = packets.try_recv().unwrap() else { panic!("missing resumed map") };
    let map = GetExploreInfoReply::decode(&*body).unwrap().explore_info.unwrap().explore_map.unwrap();
    assert_eq!((map.posx, map.posy), (Some(1), Some(5)));

    dispatch_command(&mut ctx, request(CmdId::ExploreMoveCmd, ExploreMoveRequest { posx: Some(99), posy: Some(99), interact_id: None }, 5)).await.unwrap();
    let CommandPacket::Push { cmd_id: CmdId::ServerErrorInfoPushCmd, .. } = packets.try_recv().unwrap() else { panic!("invalid move needs an error") };
    let CommandPacket::Reply { cmd_id: CmdId::ExploreMoveCmd, result_code, .. } = packets.try_recv().unwrap() else { panic!("missing rejected movement reply") };
    assert_ne!(result_code, 0);
    assert!(!ctx.should_disconnect());
}
