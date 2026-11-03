use super::*;
use sonettobuf::{ChangeMapReply, ChangeMapRequest, GetExploreInfoReply, GetExploreInfoRequest};

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
