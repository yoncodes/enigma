use super::*;
use sonettobuf::{ChangeMapReply, ChangeMapRequest, GetExploreInfoReply, GetExploreInfoRequest};
use sonettobuf::{ExploreMoveReply, ExploreMoveRequest, ExploreStepPush};

#[tokio::test]
async fn exploration_routes_entry_and_resume() {
    config::init(&format!(
        "{}/../data/excel2json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (938, 'explore-route', 0, 0)")
        .execute(&pool).await.unwrap();
    database::db::game::open_infos::ensure_open_infos(
        &pool,
        938,
        &[database::db::game::open_infos::OpenInfoSeed {
            id: 47,
            is_open: true,
        }],
    )
    .await
    .unwrap();
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(4);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(938, PlayerState::new(938, 0)));
    for (cmd_id, data, up_tag) in [
        (
            CmdId::ChangeMapCmd,
            ChangeMapRequest { map_id: Some(101) }.encode_to_vec(),
            8,
        ),
        (
            CmdId::GetExploreInfoCmd,
            GetExploreInfoRequest {}.encode_to_vec(),
            9,
        ),
    ] {
        let request = ClientPacket {
            sequence: 1,
            cmd_id: cmd_id as i16,
            up_tag,
            data,
        }
        .encode();
        dispatch_command(&mut ctx, request).await.unwrap();
        let CommandPacket::Reply {
            cmd_id: actual,
            body,
            result_code,
            up_tag: tag,
            ..
        } = packets.try_recv().unwrap()
        else {
            panic!("exploration reply missing");
        };
        assert_eq!((actual, result_code, tag), (cmd_id, 0, up_tag));
        let info = if cmd_id == CmdId::ChangeMapCmd {
            ChangeMapReply::decode(&*body)
                .unwrap()
                .explore_info
                .unwrap()
        } else {
            GetExploreInfoReply::decode(&*body)
                .unwrap()
                .explore_info
                .unwrap()
        };
        assert_eq!(info.explore_map.unwrap().map_id, Some(101));
        assert!(packets.try_recv().is_err());
    }
}

#[tokio::test]
async fn exploration_move_pushes_committed_steps_before_reply() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 582;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'explore-move-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    let saved = database::models::game::explore::state::MapState {
        map_id: 101,
        posx: 48,
        posy: 23,
        dir: 270,
        challenge_count: 0,
        interacts: configs::get()
            .explore_map
            .get(101)
            .unwrap()
            .areas
            .values()
            .flat_map(|area| &area.units)
            .map(
                |unit| database::models::game::explore::state::InteractState {
                    id: unit.id,
                    kind: unit.kind,
                    status: unit.interact,
                    status2: String::new(),
                    posx: unit.node_pos[0],
                    posy: unit.node_pos[1],
                    dir: unit.unit_dir,
                },
            )
            .collect(),
        area_ids: vec![1],
        move_nodes: "1#6|48#23".into(),
    };
    let mut conn = pool.acquire().await.unwrap();
    database::db::game::explore::state::insert(&mut conn, player_id, &saved)
        .await
        .unwrap();
    database::db::game::explore::state::select(&mut conn, player_id, 101)
        .await
        .unwrap();
    drop(conn);

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(3);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));
    let mut data = Vec::new();
    ExploreMoveRequest {
        posx: Some(52),
        posy: Some(23),
        interact_id: None,
    }
    .encode(&mut data)
    .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::ExploreMoveCmd as i16,
            up_tag: 24,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("exploration move did not push committed steps first");
    };
    assert_eq!(cmd_id, CmdId::ExploreStepPushCmd);
    assert_eq!(
        ExploreStepPush::decode(&*body)
            .unwrap()
            .steps
            .into_iter()
            .map(|step| step.param.unwrap())
            .collect::<Vec<_>>(),
        [
            r#"{"direction":90,"stepType":1,"x":49,"y":23}"#,
            r#"{"direction":90,"stepType":1,"x":50,"y":23}"#,
            r#"{"direction":90,"stepType":1,"x":51,"y":23}"#,
            r#"{"direction":90,"stepType":1,"x":52,"y":23}"#,
        ]
    );
    let CommandPacket::Reply {
        cmd_id,
        body,
        up_tag,
        result_code,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("exploration move did not reply last");
    };
    assert_eq!(cmd_id, CmdId::ExploreMoveCmd);
    assert_eq!(up_tag, 24);
    assert_eq!(result_code, 0);
    ExploreMoveReply::decode(&*body).unwrap();
    assert!(packets.try_recv().is_err());
}
