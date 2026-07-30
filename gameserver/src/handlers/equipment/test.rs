use super::on_equip_lock;
use crate::{
    net::{
        app::AppState, context::ConnectionContext, outbound::CommandPacket, packet::ClientPacket,
    },
    player::{Player, PlayerState},
};
use config::configs;
use prost::Message;
use sonettobuf::{CmdId, EquipLockReply, EquipLockRequest, EquipUpdatePush};
use sqlx::SqlitePool;
use tokio::sync::mpsc;

#[tokio::test]
async fn equip_lock_pushes_committed_state_before_reply() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (26, 'equip-lock', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO equipment
         (uid, user_id, equip_id, level, exp, break_lv, count, is_lock, refine_lv,
          created_at, updated_at)
         VALUES (40, 26, 1571, 1, 0, 0, 1, 0, 1, 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(2);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(26, PlayerState::new(26, 0)));
    let mut data = Vec::new();
    EquipLockRequest {
        target_uid: Some(40),
        lock: Some(true),
    }
    .encode(&mut data)
    .unwrap();

    on_equip_lock(
        &mut ctx,
        ClientPacket {
            sequence: 0,
            cmd_id: CmdId::EquipLockCmd as i16,
            up_tag: 7,
            data,
        },
    )
    .await
    .unwrap();

    let CommandPacket::Push {
        cmd_id: CmdId::EquipUpdatePushCmd,
        body,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("equipment update was not sent first");
    };
    let update = EquipUpdatePush::decode(&*body).unwrap();
    assert_eq!(update.equips.len(), 1);
    assert_eq!(update.equips[0].uid, Some(40));
    assert_eq!(update.equips[0].is_lock, Some(true));

    let CommandPacket::Reply {
        cmd_id: CmdId::EquipLockCmd,
        body,
        up_tag: 7,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("equipment lock reply was not sent second");
    };
    let reply = EquipLockReply::decode(&*body).unwrap();
    assert_eq!(reply.target_uid, Some(40));
    assert_eq!(reply.lock, Some(true));
}
