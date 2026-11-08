use super::*;

#[tokio::test]
async fn act107_goods_info_matches_captured_store() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 5616;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'act107-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(2);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));
    let mut data = Vec::new();
    Get107GoodsInfoRequest {
        activity_id: Some(13903),
    }
    .encode(&mut data)
    .unwrap();

    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::Get107GoodsInfoCmd as i16,
            up_tag: 58,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();

    let CommandPacket::Reply {
        cmd_id,
        body,
        result_code: 0,
        up_tag,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("activity 107 goods request did not reach its handler");
    };
    assert_eq!(cmd_id, CmdId::Get107GoodsInfoCmd);
    assert_eq!(up_tag, 58);
    let reply = Get107GoodsInfoReply::decode(&*body).unwrap();
    assert_eq!(reply.activity_id, Some(13903));
    assert_eq!(reply.goods_infos.len(), 32);
    let mut ids = reply
        .goods_infos
        .iter()
        .filter_map(|goods| goods.id)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    assert_eq!(
        ids,
        vec![
            101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 112, 113, 201, 202, 203, 204, 205,
            206, 207, 208, 209, 210, 211, 212, 213, 214, 301, 302, 303, 304, 305, 401,
        ]
    );
    assert!(
        reply
            .goods_infos
            .iter()
            .all(|goods| goods.buy_count == Some(0))
    );
    assert!(packets.try_recv().is_err());
}
