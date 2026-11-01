use super::*;
use sonettobuf::{Buy107GoodsRequest, Get107GoodsInfoReply, Get107GoodsInfoRequest};

#[tokio::test]
async fn activity107_shop_requests_reach_the_router_and_reject_invalid_purchases() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let user = 970104;
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (?, 'activity107-router', 0, 0)")
        .bind(user).execute(&pool).await.unwrap();
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(16);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(user, PlayerState::new(user, 0)));
    let request = ClientPacket { sequence: 1, cmd_id: CmdId::Get107GoodsInfoCmd as i16, up_tag: 7,
        data: Get107GoodsInfoRequest { activity_id: Some(13803) }.encode_to_vec() }.encode();
    dispatch_command(&mut ctx, request).await.unwrap();
    let CommandPacket::Reply { cmd_id: CmdId::Get107GoodsInfoCmd, result_code: 0, body, .. } = packets.try_recv().unwrap()
        else { panic!("activity store info request did not reach the handler") };
    let reply = Get107GoodsInfoReply::decode(&*body).unwrap();
    assert_eq!(reply.activity_id, Some(13803));
    assert_eq!(reply.goods_infos.iter().find(|row| row.id == Some(104)).unwrap().buy_count, Some(0));
    let request = ClientPacket { sequence: 2, cmd_id: CmdId::Buy107GoodsCmd as i16, up_tag: 8,
        data: Buy107GoodsRequest { activity_id: Some(13803), id: Some(104), num: Some(0) }.encode_to_vec() }.encode();
    dispatch_command(&mut ctx, request).await.unwrap();
    let CommandPacket::Push { cmd_id: CmdId::ServerErrorInfoPushCmd, .. } = packets.try_recv().unwrap()
        else { panic!("invalid purchase did not return an error") };
    let CommandPacket::Reply { cmd_id: CmdId::Buy107GoodsCmd, result_code, .. } = packets.try_recv().unwrap()
        else { panic!("purchase request did not reach the handler") };
    assert_ne!(result_code, 0);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_activity_state WHERE user_id = ?")
        .bind(user).fetch_one(state.db).await.unwrap();
    assert_eq!(count, 0);
}
