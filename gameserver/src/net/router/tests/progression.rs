use super::*;

#[tokio::test]
async fn settlement_acknowledgements_reach_handlers_and_clear_their_flags() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    config::init(data_dir.to_str().unwrap()).unwrap();
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 526;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'weekwalk-shallow-ack', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_weekwalk_info
         (user_id, issue_id, is_pop_deep_rule, is_pop_shallow_settle, is_pop_deep_settle)
         VALUES (?, 59, TRUE, TRUE, TRUE)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(2);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    for (sequence, up_tag) in [(1, 41), (2, 42)] {
        let mut data = Vec::new();
        MarkPopShallowSettleRequest {}.encode(&mut data).unwrap();
        let request = ClientPacket {
            sequence,
            cmd_id: CmdId::MarkPopShallowSettleCmd as i16,
            up_tag,
            data,
        }
        .encode();

        dispatch_command(&mut ctx, request).await.unwrap();

        let CommandPacket::Reply {
            cmd_id: CmdId::MarkPopShallowSettleCmd,
            body,
            up_tag: reply_up_tag,
            ..
        } = packets.try_recv().unwrap()
        else {
            panic!("shallow-settlement acknowledgement did not reach its handler");
        };
        assert_eq!(reply_up_tag, up_tag);
        MarkPopShallowSettleReply::decode(&*body).unwrap();
        assert!(packets.try_recv().is_err());
    }

    let info = database::db::game::weekwalk::get_weekwalk_info(ctx.state.db, player_id)
        .await
        .unwrap()
        .0;
    assert_eq!(info.issue_id, 59);
    assert!(!info.is_pop_shallow_settle);
    assert!(info.is_pop_deep_rule);
    assert!(info.is_pop_deep_settle);

    let mut data = Vec::new();
    MarkPopDeepSettleRequest {}.encode(&mut data).unwrap();
    let request = ClientPacket {
        sequence: 3,
        cmd_id: CmdId::MarkPopDeepSettleCmd as i16,
        up_tag: 43,
        data,
    }
    .encode();

    dispatch_command(&mut ctx, request).await.unwrap();

    let CommandPacket::Reply {
        cmd_id: CmdId::MarkPopDeepSettleCmd,
        body,
        result_code: 0,
        up_tag: 43,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("deep-settlement acknowledgement did not reach its handler");
    };
    MarkPopDeepSettleReply::decode(&*body).unwrap();
    assert!(packets.try_recv().is_err());

    let info = database::db::game::weekwalk::get_weekwalk_info(ctx.state.db, player_id)
        .await
        .unwrap()
        .0;
    assert!(!info.is_pop_shallow_settle);
    assert!(!info.is_pop_deep_settle);
}

#[tokio::test]
async fn act220_info_command_reaches_handler_and_decodes_captured_initial_episode() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let activity_id = 13710;
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 5619;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'act220-info', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(1);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    let mut data = Vec::new();
    GetAct220InfoRequest {
        activity_id: Some(activity_id),
    }
    .encode(&mut data)
    .unwrap();
    let request = ClientPacket {
        sequence: 1,
        cmd_id: CmdId::GetAct220InfoCmd as i16,
        up_tag: 52,
        data,
    }
    .encode();

    dispatch_command(&mut ctx, request).await.unwrap();

    let CommandPacket::Reply {
        cmd_id: CmdId::GetAct220InfoCmd,
        body,
        result_code: 0,
        up_tag: 52,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Act220 information request did not reach its handler");
    };
    let reply = GetAct220InfoReply::decode(&*body).unwrap();
    assert_eq!(
        reply,
        GetAct220InfoReply {
            activity_id: Some(activity_id),
            episodes: vec![Act220EpisodeRecord {
                episode_id: Some(1371001),
                is_finished: Some(false),
                unlock_branch_ids: Vec::new(),
                progress: Some(String::new()),
            }],
        }
    );
    assert!(packets.try_recv().is_err());
}

#[tokio::test]
async fn act220_finish_episode_matches_captured_progression_sequence() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 5620;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'act220-finish', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    database::db::game::player_infos::create_player_info(&pool, player_id, 0)
        .await
        .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(32);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(
        player_id,
        PlayerState::new(player_id, ::common::time::ServerTime::now_ms()),
    ));
    let mut data = Vec::new();
    Act220FinishEpisodeRequest {
        activity_id: Some(13904),
        episode_id: Some(1390401),
        progress: Some(String::new()),
    }
    .encode(&mut data)
    .unwrap();

    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::Act220FinishEpisodeCmd as i16,
            up_tag: 61,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Act220 successor episode was not pushed");
    };
    assert_eq!(cmd_id, CmdId::Act220EpisodePushCmd);
    assert_eq!(
        Act220EpisodePush::decode(&*body).unwrap(),
        Act220EpisodePush {
            activity_id: Some(13904),
            episodes: vec![Act220EpisodeRecord {
                episode_id: Some(1390402),
                is_finished: Some(false),
                unlock_branch_ids: Vec::new(),
                progress: Some(String::new()),
            }],
        }
    );

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Act220 task update was not pushed");
    };
    assert_eq!(cmd_id, CmdId::UpdateTaskPushCmd);
    let task_update = UpdateTaskPush::decode(&*body).unwrap();
    assert_eq!(task_update.task_info.len(), 1);
    let task = &task_update.task_info[0];
    assert_eq!(task.id, 690601);
    assert_eq!(task.progress, 1);
    assert!(task.has_finished);
    assert_eq!(task.r#type, Some(71));

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Act220 task red dot was not pushed");
    };
    assert_eq!(cmd_id, CmdId::UpdateRedDotPushCmd);
    let red_dot = UpdateRedDotPush::decode(&*body).unwrap();
    assert_eq!(red_dot.red_dot_infos.len(), 1);
    let group = &red_dot.red_dot_infos[0];
    assert_eq!(group.define_id, 3215);
    assert_eq!(group.replace_all, Some(true));
    assert_eq!(group.infos.len(), 1);
    assert_eq!(group.infos[0].id, 13904);
    assert_eq!(group.infos[0].value, 1);
    assert_eq!(group.infos[0].time, Some(0));

    let CommandPacket::Reply {
        cmd_id,
        body,
        result_code: 0,
        up_tag: 61,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Act220 finish reply did not reach the client");
    };
    assert_eq!(cmd_id, CmdId::Act220FinishEpisodeCmd);
    assert_eq!(
        Act220FinishEpisodeReply::decode(&*body).unwrap(),
        Act220FinishEpisodeReply {
            activity_id: Some(13904),
            episode_id: Some(1390401),
            progress: Some(String::new()),
        }
    );
    assert!(packets.try_recv().is_err());

    let mut data = Vec::new();
    FinishTaskRequest { id: 690601 }.encode(&mut data).unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 2,
            cmd_id: CmdId::FinishTaskCmd as i16,
            up_tag: 62,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();

    let mut claim_result = None;
    let mut cleared_red_dot = false;
    while let Ok(packet) = packets.try_recv() {
        match packet {
            CommandPacket::Reply {
                cmd_id: CmdId::FinishTaskCmd,
                result_code,
                up_tag,
                ..
            } => {
                assert_eq!(up_tag, 62);
                claim_result = Some(result_code);
            }
            CommandPacket::Push {
                cmd_id: CmdId::UpdateRedDotPushCmd,
                body,
                ..
            } => {
                let red_dot = UpdateRedDotPush::decode(&*body).unwrap();
                cleared_red_dot = red_dot.red_dot_infos.iter().any(|group| {
                    group.define_id == 3215
                        && group.replace_all == Some(true)
                        && group.infos.len() == 1
                        && group.infos[0].id == 13904
                        && group.infos[0].value == 0
                        && group.infos[0].time == Some(0)
                });
            }
            _ => {}
        }
    }
    assert_eq!(claim_result, Some(0));
    assert!(cleared_red_dot);
}

#[tokio::test]
async fn rouge2_outside_info_command_decodes_captured_initial_boss_state() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 534;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'rouge2-boss-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(1);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    let mut data = Vec::new();
    GetRouge2OutsideInfoRequest {}.encode(&mut data).unwrap();
    let request = ClientPacket {
        sequence: 1,
        cmd_id: CmdId::GetRouge2OutsideInfoCmd as i16,
        up_tag: 54,
        data,
    }
    .encode();

    dispatch_command(&mut ctx, request).await.unwrap();

    let CommandPacket::Reply {
        cmd_id: CmdId::GetRouge2OutsideInfoCmd,
        body,
        result_code: 0,
        up_tag: 54,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Rouge2 outside information request did not reach its handler");
    };
    let reply = GetRouge2OutsideInfoReply::decode(&*body).unwrap();
    let mut expected_career_levels = configs::get()
        .rouge2_career
        .iter()
        .map(|row| Rouge2CareerLevelInfo {
            career_id: Some(row.id),
            exp: Some(0),
        })
        .collect::<Vec<_>>();
    expected_career_levels.sort_by_key(|row| row.career_id);
    let mut expected_rewards = configs::get()
        .rouge2_reward
        .iter()
        .map(|row| Rouge2RewardInfo {
            id: Some(row.id),
            buy_count: Some(0),
        })
        .collect::<Vec<_>>();
    expected_rewards.sort_by_key(|row| row.id);
    assert_eq!(
        reply,
        GetRouge2OutsideInfoReply {
            outside_info: Some(Rouge2OutsideInfo {
                genius_point: Some(0),
                genius_ids: Vec::new(),
                total_record_info: Some(Rouge2TotalRecordInfo {
                    max_difficulty: Some(0),
                    pass_layer_id: Vec::new(),
                    pass_event_id: Vec::new(),
                    pass_end_id: Vec::new(),
                    pass_entrust_id: Vec::new(),
                    last_game_time: Some(0),
                    pass_collections: Vec::new(),
                    hotfix_str: Some(String::new()),
                }),
                career_level_info: expected_career_levels,
                reward_info: expected_rewards,
                reward_point: Some(0),
                alchemy_info: Some(Rouge2AlchemyInfo {
                    cur_alchemy_info: None,
                    alchemy_material_info: Vec::new(),
                }),
                review: Vec::new(),
                boss_battle_info: Some(Rouge2BossBattleInfo {
                    boss_info: Vec::new(),
                    save_info: Vec::new(),
                    use_save_index: Some(0),
                }),
            }),
        }
    );
    assert!(packets.try_recv().is_err());
}
