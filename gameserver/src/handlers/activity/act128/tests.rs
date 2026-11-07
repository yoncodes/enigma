use super::*;
use crate::{
    net::{app::AppState, outbound::CommandPacket, router::dispatch_command},
    player::{Player, PlayerState},
};
use database::db::game::activity_state::{self, ActivityStateKind, ActivityStateSet};
use sonettobuf::{
    Act128ChangeActModeReply, Act128ChangeActModeRequest, Act128GetExpReply, CurrencyChangePush,
    Get128InfosReply, Get128InfosRequest, GetGalleryInfosReply,
};
use sqlx::SqlitePool;
use tokio::sync::mpsc;

#[tokio::test]
async fn change_mode_routes_and_persists_in_activity_info() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (81, 'act128-mode', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let activity_id = config::configs::get().latest_open_activity_id(128).unwrap();
    let state = Box::leak(Box::new(AppState::new(pool, config::configs::get())));
    let (outbound, mut packets) = mpsc::channel(4);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(81, PlayerState::new(81, 0)));

    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::Act128ChangeActModeCmd as i16,
            up_tag: 41,
            data: Act128ChangeActModeRequest {
                activity_id: Some(activity_id),
                is_activity_mode_open: Some(true),
            }
            .encode_to_vec(),
        }
        .encode(),
    )
    .await
    .unwrap();
    let CommandPacket::Reply { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("mode request did not reach its handler");
    };
    assert_eq!(cmd_id, CmdId::Act128ChangeActModeCmd);
    let reply = Act128ChangeActModeReply::decode(&*body).unwrap();
    assert_eq!(reply.activity_id, Some(activity_id));
    assert_eq!(reply.is_activity_mode_open, Some(true));

    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 2,
            cmd_id: CmdId::Get128InfosCmd as i16,
            up_tag: 42,
            data: Get128InfosRequest {
                activity_id: Some(activity_id),
            }
            .encode_to_vec(),
        }
        .encode(),
    )
    .await
    .unwrap();
    let CommandPacket::Reply { body, .. } = packets.try_recv().unwrap() else {
        panic!("activity info request did not reach its handler");
    };
    assert_eq!(
        Get128InfosReply::decode(&*body)
            .unwrap()
            .is_activity_mode_open,
        Some(true)
    );
}

#[tokio::test]
async fn gallery_commands_route_and_send_currency_before_the_claim_reply() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at) VALUES (80, 'gallery', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO currencies (user_id, currency_id, quantity) VALUES (80, 3206, 700)")
        .execute(&pool)
        .await
        .unwrap();
    activity_state::set(
        &pool,
        80,
        13809,
        ActivityStateSet {
            kind: ActivityStateKind::Act128BossScore,
            entry_id: 1,
            state: 8_427_039,
            progress: 8_427_039,
            ext: "",
        },
    )
    .await
    .unwrap();
    let state = Box::leak(Box::new(AppState::new(pool, config::configs::get())));
    let (outbound, mut packets) = mpsc::channel(8);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(80, PlayerState::new(80, 0)));
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::GetGalleryInfosCmd as i16,
            up_tag: 31,
            data: GetGalleryInfosRequest {
                activity_id: Some(13809),
            }
            .encode_to_vec(),
        }
        .encode(),
    )
    .await
    .unwrap();
    let CommandPacket::Reply {
        cmd_id: CmdId::GetGalleryInfosCmd,
        body,
        result_code: 0,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Gallery request did not reach its handler");
    };
    let info = GetGalleryInfosReply::decode(&*body).unwrap();
    assert_eq!(
        info.gallery_detail
            .iter()
            .find(|entry| entry.r#type == Some(109))
            .unwrap()
            .highest_point,
        Some(8_427_039)
    );
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 2,
            cmd_id: CmdId::Act128GetExpCmd as i16,
            up_tag: 32,
            data: Act128GetExpRequest {
                activity_id: Some(13809),
                r#type: Some(109),
            }
            .encode_to_vec(),
        }
        .encode(),
    )
    .await
    .unwrap();
    let events = std::iter::from_fn(|| packets.try_recv().ok()).collect::<Vec<_>>();
    let currency = events
        .iter()
        .position(|event| {
            matches!(
                event,
                CommandPacket::Push {
                    cmd_id: CmdId::CurrencyChangePushCmd,
                    ..
                }
            )
        })
        .unwrap();
    let reply = events
        .iter()
        .position(|event| {
            matches!(
                event,
                CommandPacket::Reply {
                    cmd_id: CmdId::Act128GetExpCmd,
                    result_code: 0,
                    up_tag: 32,
                    ..
                }
            )
        })
        .unwrap();
    assert!(currency < reply);
    let CommandPacket::Push { body, .. } = &events[currency] else {
        unreachable!()
    };
    let changed = CurrencyChangePush::decode(&**body).unwrap();
    assert_eq!(
        (
            changed.change_currency[0].currency_id,
            changed.change_currency[0].quantity
        ),
        (Some(3206), Some(1150))
    );
    let CommandPacket::Reply { body, .. } = &events[reply] else {
        unreachable!()
    };
    let claim = Act128GetExpReply::decode(&**body).unwrap();
    assert_eq!(
        (claim.accept_exp_point, claim.player_exp, claim.player_level),
        (Some(7_500_000), Some(1150), Some(11))
    );
}
