use super::*;

#[tokio::test]
async fn shallow_settlement_ack_reaches_handler_and_only_clears_its_flag() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
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

    let info = ctx
        .player()
        .unwrap()
        .exploration
        .weekwalk_info(ctx.state.db)
        .await
        .unwrap()
        .info
        .unwrap();
    assert_eq!(info.issue_id, Some(59));
    assert_eq!(info.is_pop_shallow_settle, Some(false));
    assert_eq!(info.is_pop_deep_rule, Some(true));
    assert_eq!(info.is_pop_deep_settle, Some(true));
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
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(1);
    let mut ctx = ConnectionContext::new(outbound, state);

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
    let mut expected_materials = configs::get()
        .rouge2_material
        .iter()
        .map(|row| Rouge2AlchemyMaterialInfo {
            id: Some(row.id),
            num: Some(0),
        })
        .collect::<Vec<_>>();
    expected_materials.sort_by_key(|row| row.id);
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
                    alchemy_material_info: expected_materials,
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
