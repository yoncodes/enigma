use super::*;
use crate::{
    net::{app::AppState, outbound::CommandPacket},
    player::{Player, PlayerState},
};
use database::db::game::activity101;
use sqlx::SqlitePool;
use tokio::sync::mpsc;

#[tokio::test]
async fn reconnect_advances_activity101_once_per_server_day() {
    crate::net::app::init_test_server_config();
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 540;
    let activity_id = 13805;
    let event_start = ::common::activity_schedule()
        .iter()
        .find(|schedule| schedule.id == activity_id && schedule.enabled)
        .unwrap()
        .start_time as i64;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
             VALUES (?, 'act101-reconnect', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO player_state
             (player_id, created_at, updated_at, last_daily_reset_time,
              last_weekly_reset_time, last_monthly_reset_time)
             VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(player_id)
    .bind(event_start)
    .bind(event_start)
    .bind(event_start)
    .bind(event_start)
    .bind(event_start)
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, config::configs::get())));
    let (outbound, mut packets) = mpsc::channel(3);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(
        player_id,
        PlayerState::new(player_id, event_start),
    ));

    for (sequence, now_ms) in [event_start, event_start + 1_000, event_start + 86_400_000]
        .into_iter()
        .enumerate()
    {
        reconnect_at(
            &mut ctx,
            ClientPacket {
                sequence: sequence as i32,
                cmd_id: CmdId::ReconnectCmd as i16,
                up_tag: sequence as u8,
                data: Vec::new(),
            },
            now_ms,
        )
        .await
        .unwrap();
        assert!(matches!(
            packets.try_recv().unwrap(),
            CommandPacket::Reply {
                cmd_id: CmdId::ReconnectCmd,
                result_code: 0,
                ..
            }
        ));
    }

    assert_eq!(
        activity101::get_activity101_info(ctx.state.db, player_id, activity_id)
            .await
            .unwrap()
            .1,
        2
    );
}
