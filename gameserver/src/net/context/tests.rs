use super::*;
use crate::{
    handlers::{dungeon::on_reconnect_fight, system::on_login},
    net::{outbound::CommandPacket, packet::ClientPacket},
    player::battle::ActiveBattle,
};
use prost::Message;
use sonettobuf::{CmdId, ReconnectFightReply, ReconnectFightRequest};
use sqlx::SqlitePool;

#[tokio::test]
async fn replacement_login_waits_for_active_battle_handoff_before_reconnect() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (7, 'reconnect', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    database::db::starter_data::load_all_starter_data(&pool, 7)
        .await
        .unwrap();

    let app: &'static AppState = Box::leak(Box::new(AppState::new(pool, config::configs::get())));
    let session_lock = app.lock_session(7).await;
    let (outbound, _) = mpsc::channel(1);
    let mut old = ConnectionContext::new(outbound, app);
    old.load_player(7).await.unwrap();
    let active = ActiveBattle {
        episode_id: 13_500_413,
        battle_id: 103_370_111,
        ..Default::default()
    };
    let expected = active.reconnect_reply();
    old.player_mut().unwrap().battle.restore_active(active);
    old.register_with_session_lock(session_lock);

    let (attempting_login, login_attempted) = tokio::sync::oneshot::channel();
    let replacement = tokio::spawn(async move {
        attempting_login.send(()).unwrap();
        let session_lock = app.lock_session(7).await;
        let (outbound, packets) = mpsc::channel(1);
        let mut ctx = ConnectionContext::new(outbound, app);
        ctx.load_player(7).await.unwrap();
        ctx.register_with_session_lock(session_lock);
        (ctx, packets)
    });
    login_attempted.await.unwrap();
    tokio::task::yield_now().await;
    assert!(!replacement.is_finished());

    let old_outbound = old.outbound.clone();
    old.park_active_battle();
    assert!(app.unregister_session_if_current(7, &old_outbound));
    drop(old);

    let (mut ctx, mut packets) = replacement.await.unwrap();
    let current_outbound = ctx.outbound.clone();
    let repeated_login = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        on_login(
            &mut ctx,
            ClientPacket {
                sequence: 0,
                cmd_id: CmdId::LoginCmd as i16,
                up_tag: 2,
                data: Vec::new(),
            },
        ),
    )
    .await
    .unwrap();
    assert!(matches!(repeated_login, Err(AppError::InvalidRequest)));
    assert!(app.is_current_session(7, &current_outbound));
    assert!(ctx.session_lock.is_some());

    on_reconnect_fight(
        &mut ctx,
        ClientPacket {
            sequence: 0,
            cmd_id: CmdId::ReconnectFightCmd as i16,
            up_tag: 3,
            data: ReconnectFightRequest::default().encode_to_vec(),
        },
    )
    .await
    .unwrap();
    let actual = match packets.recv().await.unwrap() {
        CommandPacket::Reply { cmd_id, body, .. } => {
            assert_eq!(cmd_id, CmdId::ReconnectFightCmd);
            ReconnectFightReply::decode(body.as_slice()).unwrap()
        }
        CommandPacket::Push { .. } => panic!("expected reconnect reply"),
    };
    assert_eq!(actual, expected);
}
