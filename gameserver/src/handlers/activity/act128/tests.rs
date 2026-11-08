use super::*;
use crate::{
    net::{app::AppState, outbound::CommandPacket, router::dispatch_command},
    player::{Player, PlayerState},
};
use database::db::game::activity_state::{self, ActivityStateKind, ActivityStateSet};
use database::models::game::heros::UserHeroModel;
use sonettobuf::{
    Act128ActModeHeroSlot, Act128ChangeActModeReply, Act128ChangeActModeRequest, Act128GetExpReply,
    CurrencyChangePush, Get128InfosReply, Get128InfosRequest, GetGalleryInfosReply,
    ResetAct128TeamReply, ResetAct128TeamRequest, SetAct128TeamReply, SetAct128TeamRequest,
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
async fn mode_team_commands_route_persist_and_reset() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (82, 'act128-team', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let tables = config::configs::get();
    let hero = tables.character.get(3146).unwrap();
    let boost = tables.character.get(3134).unwrap();
    let bond = tables
        .activity128_expand_bond
        .iter()
        .find(|row| {
            row.tag_type == 2
                && row
                    .tag2
                    .split('|')
                    .any(|tag| boost.battle_tag.split('#').any(|hero_tag| hero_tag == tag))
        })
        .unwrap();
    let boss_id = tables
        .activity128_episode
        .iter()
        .find(|row| row.activity_id == bond.activity_id)
        .unwrap()
        .stage;
    let hero_uid = UserHeroModel::new(82, pool.clone())
        .create_hero(hero.id)
        .await
        .unwrap();
    let boost_uid = UserHeroModel::new(82, pool.clone())
        .create_hero(boost.id)
        .await
        .unwrap();
    let mut front_hero_slots = vec![Act128ActModeHeroSlot::default(); 4];
    front_hero_slots[0].uid = Some(hero_uid);
    front_hero_slots[0].is_friend = Some(false);
    front_hero_slots[0].friend_user_id = Some(0);
    front_hero_slots[1].uid = Some(boost_uid);
    front_hero_slots[1].is_friend = Some(false);
    front_hero_slots[1].friend_user_id = Some(0);
    let state = Box::leak(Box::new(AppState::new(pool, tables)));
    let (outbound, mut packets) = mpsc::channel(8);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(82, PlayerState::new(82, 0)));

    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::SetAct128TeamCmd as i16,
            up_tag: 43,
            data: SetAct128TeamRequest {
                activity_id: Some(bond.activity_id),
                boss_id: Some(boss_id),
                front_hero_slots,
                back_hero_slots: vec![Act128ActModeHeroSlot::default(); 4],
                add_bond_group_id: Some(bond.group_id),
            }
            .encode_to_vec(),
        }
        .encode(),
    )
    .await
    .unwrap();
    let CommandPacket::Reply { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("team request did not reach its handler");
    };
    assert_eq!(cmd_id, CmdId::SetAct128TeamCmd);
    let set = SetAct128TeamReply::decode(&*body).unwrap();
    let team = set.act_mode_team.unwrap();
    assert_eq!(team.front_hero_infos.len(), 4);
    assert_eq!(team.front_hero_infos[0].uid, Some(hero_uid));
    assert_eq!(team.add_bond_group_id, Some(bond.group_id));

    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 2,
            cmd_id: CmdId::Get128InfosCmd as i16,
            up_tag: 44,
            data: Get128InfosRequest {
                activity_id: Some(bond.activity_id),
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
    let info = Get128InfosReply::decode(&*body).unwrap();
    let stored = info
        .boss_detail
        .iter()
        .find(|detail| detail.boss_id == Some(boss_id))
        .unwrap()
        .act_mode_team
        .as_ref()
        .unwrap();
    assert_eq!(stored.front_hero_infos[0].uid, Some(hero_uid));

    sqlx::query(
        "UPDATE user_activity_state SET state = 123
         WHERE user_id = 82 AND activity_id = ? AND kind = ? AND entry_id = ?",
    )
    .bind(bond.activity_id)
    .bind(ActivityStateKind::Act128ModeTeam.id())
    .bind(boss_id)
    .execute(ctx.state.db)
    .await
    .unwrap();

    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 3,
            cmd_id: CmdId::ResetAct128TeamCmd as i16,
            up_tag: 45,
            data: ResetAct128TeamRequest {
                activity_id: Some(bond.activity_id),
                boss_id: Some(boss_id),
            }
            .encode_to_vec(),
        }
        .encode(),
    )
    .await
    .unwrap();
    let CommandPacket::Reply { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("reset request did not reach its handler");
    };
    assert_eq!(cmd_id, CmdId::ResetAct128TeamCmd);
    let reset = ResetAct128TeamReply::decode(&*body).unwrap();
    assert_eq!(reset.latest_point, Some(0));
    assert_eq!(reset.act_mode_team, Some(Default::default()));

    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 4,
            cmd_id: CmdId::Get128InfosCmd as i16,
            up_tag: 46,
            data: Get128InfosRequest {
                activity_id: Some(bond.activity_id),
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
    let info = Get128InfosReply::decode(&*body).unwrap();
    let reset_detail = info
        .boss_detail
        .iter()
        .find(|detail| detail.boss_id == Some(boss_id))
        .unwrap();
    assert_eq!(reset_detail.latest_point, Some(0));
    assert_eq!(reset_detail.act_mode_team, Some(Default::default()));
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
