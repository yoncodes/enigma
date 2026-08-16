use super::*;

#[tokio::test]
async fn arcade_outside_get_and_talent_commands_reach_handlers_in_capture_order() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 5655;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'arcade-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let activity_id = configs::get().latest_open_activity_id(222).unwrap();
    let (outbound, mut packets) = mpsc::channel(8);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    let mut data = Vec::new();
    ArcadeGetOutSideInfoRequest::default()
        .encode(&mut data)
        .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::ArcadeGetOutSideInfoCmd as i16,
            up_tag: 81,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();
    let CommandPacket::Reply { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Arcade outside information request did not reach its handler");
    };
    assert_eq!(cmd_id, CmdId::ArcadeGetOutSideInfoCmd);
    assert_eq!(
        ArcadeGetOutSideInfoReply::decode(&*body)
            .unwrap()
            .info
            .unwrap()
            .player
            .unwrap()
            .id,
        Some(101)
    );

    sqlx::query(
        "INSERT INTO user_arcade_attrs
         (user_id, activity_id, attr_id, base, rate, extra)
         VALUES (?, ?, 202, 240, 0, 0)",
    )
    .bind(player_id)
    .bind(activity_id)
    .execute(state.db)
    .await
    .unwrap();
    let mut data = Vec::new();
    ArcadeTalentUpgradeRequest {
        talent_id: Some(100),
        level: Some(0),
    }
    .encode(&mut data)
    .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 2,
            cmd_id: CmdId::ArcadeTalentUpgradeCmd as i16,
            up_tag: 82,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Arcade talent upgrade did not emit its attribute push");
    };
    assert_eq!(cmd_id, CmdId::ArcadeAttrChangePushCmd);
    let attr = ArcadeAttrChangePush::decode(&*body).unwrap().attr[0];
    assert_eq!(attr.base, Some(190));
    assert_eq!(attr.extra, None);
    let CommandPacket::Reply { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Arcade talent upgrade did not emit its reply");
    };
    assert_eq!(cmd_id, CmdId::ArcadeTalentUpgradeCmd);
    assert_eq!(
        ArcadeTalentUpgradeReply::decode(&*body).unwrap().level,
        Some(1)
    );
    assert!(packets.try_recv().is_err());

    sqlx::query(
        "UPDATE user_arcade_outside SET score = 1000
         WHERE user_id = ? AND activity_id = ?",
    )
    .bind(player_id)
    .bind(activity_id)
    .execute(state.db)
    .await
    .unwrap();
    let mut data = Vec::new();
    ArcadeGainRewardRequest { reward_id: Some(0) }
        .encode(&mut data)
        .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 3,
            cmd_id: CmdId::ArcadeGainRewardCmd as i16,
            up_tag: 83,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();
    let mut claim_packets = Vec::new();
    while let Ok(packet) = packets.try_recv() {
        claim_packets.push(packet);
    }
    let material_index = claim_packets
        .iter()
        .position(|packet| {
            matches!(
                packet,
                CommandPacket::Push {
                    cmd_id: CmdId::MaterialChangePushCmd,
                    ..
                }
            )
        })
        .unwrap();
    let CommandPacket::Push { body, .. } = &claim_packets[material_index] else {
        unreachable!()
    };
    assert_eq!(
        MaterialChangePush::decode(&**body).unwrap().get_approach,
        Some(154)
    );
    let CommandPacket::Push { cmd_id, body, .. } = &claim_packets[material_index + 1] else {
        panic!("Arcade claim did not emit its final red-dot push");
    };
    assert_eq!(*cmd_id, CmdId::UpdateRedDotPushCmd);
    let red_dot = UpdateRedDotPush::decode(&**body).unwrap();
    assert_eq!(red_dot.red_dot_infos[0].define_id, 3306);
    assert_eq!(red_dot.red_dot_infos[0].infos[0].id, 0);
    assert_eq!(red_dot.red_dot_infos[0].replace_all, Some(true));
    let CommandPacket::Reply { cmd_id, body, .. } = &claim_packets[material_index + 2] else {
        panic!("Arcade claim did not end with its reply");
    };
    assert_eq!(*cmd_id, CmdId::ArcadeGainRewardCmd);
    assert_eq!(
        ArcadeGainRewardReply::decode(&**body).unwrap().reward_id,
        Some(0)
    );
}
