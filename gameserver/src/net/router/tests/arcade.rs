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

#[tokio::test]
async fn arcade_inside_commands_resume_and_settle_persistent_run() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 5672;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'arcade-inside-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(32);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    let mut data = Vec::new();
    ArcadeGetInSideInfoRequest::default()
        .encode(&mut data)
        .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::ArcadeGetInSideInfoCmd as i16,
            up_tag: 91,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();
    let CommandPacket::Reply { body, .. } = packets.try_recv().unwrap() else {
        panic!("Arcade inside information request did not reach its handler");
    };
    assert_eq!(
        ArcadeGetInSideInfoReply::decode(&*body)
            .unwrap()
            .has_save_game,
        Some(false)
    );

    let run = ArcadeInSideInfo {
        player: Some(ArcadePlayer {
            id: Some(101),
            ..Default::default()
        }),
        attr_container: Some(ArcadeAttrContainer {
            attr_values: vec![
                ArcadeAttrValue {
                    id: Some(202),
                    base: Some(240),
                    ..Default::default()
                },
                ArcadeAttrValue {
                    id: Some(207),
                    base: Some(2500),
                    ..Default::default()
                },
            ],
        }),
        prop: Some(ArcadeInSideProp {
            area_id: Some(0),
            room_id: Some(10001),
            progress: Some(0),
            difficulty: Some(0),
            ..Default::default()
        }),
        extend_info: Some(ArcadeExtendInfo {
            added_book: Some(ArcadeBookInfo {
                books: vec![
                    ArcadeBook {
                        r#type: Some(2),
                        ele_id: vec![10009, 10004],
                        ..Default::default()
                    },
                    ArcadeBook {
                        r#type: Some(3),
                        ele_id: vec![106],
                        ..Default::default()
                    },
                    ArcadeBook {
                        r#type: Some(4),
                        ele_id: vec![
                            200015, 200007, 210002, 200001, 210004, 200004, 200002, 200003, 200005,
                            200008, 210001, 210003, 200006,
                        ],
                        ..Default::default()
                    },
                ],
            }),
            unlock_difficulty_ids: vec![1],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut data = Vec::new();
    ArcadeSaveGameRequest {
        info: Some(run.clone()),
    }
    .encode(&mut data)
    .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 2,
            cmd_id: CmdId::ArcadeSaveGameCmd as i16,
            up_tag: 92,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();
    let CommandPacket::Reply { body, .. } = packets.try_recv().unwrap() else {
        panic!("Arcade save request did not reach its handler");
    };
    ArcadeSaveGameReply::decode(&*body).unwrap();

    let mut data = Vec::new();
    ArcadeSettleGameRequest {
        r#type: Some(2),
        info: Some(run),
    }
    .encode(&mut data)
    .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 3,
            cmd_id: CmdId::ArcadeSettleGameCmd as i16,
            up_tag: 93,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();
    let mut settlement_packets = Vec::new();
    while let Ok(packet) = packets.try_recv() {
        settlement_packets.push(packet);
    }
    let cmd_ids = settlement_packets
        .iter()
        .map(|packet| match packet {
            CommandPacket::Push { cmd_id, .. } | CommandPacket::Reply { cmd_id, .. } => *cmd_id,
        })
        .collect::<Vec<_>>();
    let mut expected_task_types = vec![79, 62];
    if database::db::game::tasks::current_battle_pass_id().is_some() {
        expected_task_types.push(10);
    }
    let mut expected_cmd_ids = Vec::new();
    for _ in &expected_task_types {
        expected_cmd_ids.push(CmdId::UpdateTaskPushCmd);
        expected_cmd_ids.push(CmdId::UpdateRedDotPushCmd);
    }
    expected_cmd_ids.extend([
        CmdId::UpdateRedDotPushCmd,
        CmdId::ArcadeAttrChangePushCmd,
        CmdId::ArcadeSettleGameCmd,
    ]);
    assert_eq!(cmd_ids, expected_cmd_ids);

    for (task_index, expected_type) in expected_task_types.iter().enumerate() {
        let CommandPacket::Push { body, .. } = &settlement_packets[task_index * 2] else {
            panic!("Arcade settlement task update was not a push");
        };
        let update = UpdateTaskPush::decode(&**body).unwrap();
        assert_eq!(update.task_info.len(), 1);
        assert_eq!(update.task_info[0].r#type, Some(*expected_type));
    }

    let mut expected_red_dots = vec![
        vec![(3705, vec![(3, 1)]), (3708, vec![(0, 0)])],
        vec![(3005, vec![(0, 1)])],
    ];
    if database::db::game::tasks::current_battle_pass_id().is_some() {
        expected_red_dots.push(vec![
            (1027, vec![(0, 0)]),
            (1047, vec![(0, 0)]),
            (2204, vec![(0, 0)]),
        ]);
    }
    expected_red_dots.push(vec![(3306, vec![(0, 1)])]);
    let red_dot_packet_indexes = (0..expected_task_types.len())
        .map(|index| index * 2 + 1)
        .chain(std::iter::once(expected_task_types.len() * 2));
    for (packet_index, expected) in red_dot_packet_indexes.zip(expected_red_dots) {
        let CommandPacket::Push { body, .. } = &settlement_packets[packet_index] else {
            panic!("Arcade settlement red-dot update was not a push");
        };
        let update = UpdateRedDotPush::decode(&**body).unwrap();
        let actual = update
            .red_dot_infos
            .iter()
            .map(|group| {
                assert_eq!(group.replace_all, Some(true));
                (
                    group.define_id,
                    group
                        .infos
                        .iter()
                        .map(|info| (info.id, info.value))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    let CommandPacket::Reply { cmd_id, body, .. } = settlement_packets.last().unwrap() else {
        panic!("Arcade settlement did not end with its reply");
    };
    assert_eq!(*cmd_id, CmdId::ArcadeSettleGameCmd);
    assert_eq!(
        ArcadeSettleGameReply::decode(&**body)
            .unwrap()
            .book_add_score,
        Some(400)
    );
}
