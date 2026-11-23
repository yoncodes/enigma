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

#[tokio::test]
async fn act107_purchase_replies_with_the_new_buy_count() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 5617;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'act107-buy', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO currencies (user_id, currency_id, quantity) VALUES (?, 3901, 8000)")
        .bind(player_id)
        .execute(&pool)
        .await
        .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(16);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));
    let mut data = Vec::new();
    Buy107GoodsRequest {
        activity_id: Some(13903),
        id: Some(101),
        num: Some(1),
    }
    .encode(&mut data)
    .unwrap();

    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::Buy107GoodsCmd as i16,
            up_tag: 59,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();

    let mut sent = Vec::new();
    while let Ok(packet) = packets.try_recv() {
        sent.push(packet);
    }
    let last = sent.pop();
    let pushes = sent
        .iter()
        .map(|packet| match packet {
            CommandPacket::Reply { cmd_id, .. } | CommandPacket::Push { cmd_id, .. } => *cmd_id,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        pushes,
        [
            CmdId::CurrencyChangePushCmd,
            CmdId::EquipUpdatePushCmd,
            CmdId::MaterialChangePushCmd,
        ]
    );
    let Some(CommandPacket::Push { body, .. }) = sent.last() else {
        panic!("activity 107 purchase did not push its material change");
    };
    let material = MaterialChangePush::decode(&**body).unwrap();
    assert_eq!(material.get_approach, Some(25));
    let Some(CommandPacket::Reply {
        cmd_id,
        body,
        result_code: 0,
        up_tag,
        ..
    }) = last
    else {
        panic!("activity 107 purchase did not end with a successful reply");
    };
    assert_eq!(cmd_id, CmdId::Buy107GoodsCmd);
    assert_eq!(up_tag, 59);
    let reply = Buy107GoodsReply::decode(&*body).unwrap();
    assert_eq!(reply.activity_id, Some(13903));
    assert_eq!(reply.num, Some(1));
    let goods = reply.goods_info.unwrap();
    assert_eq!((goods.id, goods.buy_count), (Some(101), Some(1)));
}
