use super::*;

#[tokio::test]
async fn act236_info_command_reaches_handler_and_returns_persisted_state() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 514;
    let activity_id = configs::get().latest_open_activity_id(236).unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'act236-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_activity236_state
         (user_id, activity_id, score, gain_reward_ids)
         VALUES (?, ?, ?, '[3,7]')",
    )
    .bind(player_id)
    .bind(activity_id)
    .bind(240)
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(2);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    let mut data = Vec::new();
    GetAct236InfoRequest {
        activity_id: Some(activity_id),
    }
    .encode(&mut data)
    .unwrap();
    let request = ClientPacket {
        sequence: 1,
        cmd_id: CmdId::GetAct236InfoCmd as i16,
        up_tag: 8,
        data,
    }
    .encode();

    dispatch_command(&mut ctx, request).await.unwrap();

    let CommandPacket::Reply {
        cmd_id: CmdId::GetAct236InfoCmd,
        body,
        result_code: 0,
        up_tag: 8,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Act236 information request did not reach its handler");
    };
    let reply = GetAct236InfoReply::decode(&*body).unwrap();
    assert_eq!(
        reply.info,
        Some(Act236Info {
            activity_id: Some(activity_id),
            score: Some(240),
            gain_reward_ids: vec![3, 7],
        })
    );
    assert!(packets.try_recv().is_err());
}

#[tokio::test]
async fn act236_reward_command_emits_captured_semantic_sequence() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 528;
    let activity_id = configs::get().latest_open_activity_id(236).unwrap();
    let reward_id = configs::get()
        .activity236
        .iter()
        .find(|row| row.activity_id == activity_id && row.cost == 0)
        .unwrap()
        .id;
    let red_dot_id = configs::get().activity.get(activity_id).unwrap().red_dot_id;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'act236-reward-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(8);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    let mut data = Vec::new();
    Act236GetAutoGainRewardRequest {
        activity_id: Some(activity_id),
        reward_ids: vec![reward_id],
    }
    .encode(&mut data)
    .unwrap();
    let request = ClientPacket {
        sequence: 1,
        cmd_id: CmdId::Act236GetAutoGainRewardCmd as i16,
        up_tag: 28,
        data,
    }
    .encode();

    dispatch_command(&mut ctx, request).await.unwrap();

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Act236 reward did not emit its currency snapshot first");
    };
    assert_eq!(cmd_id, CmdId::CurrencyChangePushCmd);
    let currency = CurrencyChangePush::decode(&*body).unwrap();
    assert_eq!(currency.change_currency.len(), 1);
    assert_eq!(currency.change_currency[0].currency_id, Some(2));
    assert_eq!(currency.change_currency[0].quantity, Some(100));

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Act236 reward did not emit its material delta");
    };
    assert_eq!(cmd_id, CmdId::MaterialChangePushCmd);
    let material = MaterialChangePush::decode(&*body).unwrap();
    assert_eq!(material.get_approach, Some(171));
    assert_eq!(material.data_list.len(), 1);
    assert_eq!(material.data_list[0].materil_type, Some(2));
    assert_eq!(material.data_list[0].materil_id, Some(2));
    assert_eq!(material.data_list[0].quantity, Some(100));

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Act236 reward did not emit its activity red dot");
    };
    assert_eq!(cmd_id, CmdId::UpdateRedDotPushCmd);
    let red_dot = UpdateRedDotPush::decode(&*body).unwrap();
    assert_eq!(red_dot.red_dot_infos.len(), 1);
    assert_eq!(red_dot.red_dot_infos[0].define_id, red_dot_id);
    assert_eq!(red_dot.red_dot_infos[0].replace_all, Some(true));
    assert_eq!(red_dot.red_dot_infos[0].infos[0].id, 0);
    assert_eq!(red_dot.red_dot_infos[0].infos[0].value, 0);

    let CommandPacket::Reply {
        cmd_id,
        body,
        result_code,
        up_tag,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Act236 reward did not emit its reply last");
    };
    assert_eq!(cmd_id, CmdId::Act236GetAutoGainRewardCmd);
    assert_eq!(result_code, 0);
    assert_eq!(up_tag, 28);
    let reply = Act236GetAutoGainRewardReply::decode(&*body).unwrap();
    assert_eq!(reply.activity_id, Some(activity_id));
    assert_eq!(reply.gain_reward_ids, vec![reward_id]);
    assert!(packets.try_recv().is_err());
}

#[tokio::test]
async fn completed_charge_emits_act236_state_and_claimable_rewards_before_completion() {
    const ACTIVE_TIME_MS: i64 = 1_786_615_201_000;

    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 530;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'act236-charge-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO user_stats (user_id) VALUES (?)")
        .bind(player_id)
        .execute(&pool)
        .await
        .unwrap();
    let activity_id = configs::get().latest_open_activity_id(236).unwrap();
    sqlx::query(
        "INSERT INTO user_activity236_state
         (user_id, activity_id, score, gain_reward_ids)
         VALUES (?, ?, 0, '[1]')",
    )
    .bind(player_id)
    .bind(activity_id)
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut outbound_packets) = mpsc::channel(32);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    let mut data = Vec::new();
    NewOrderRequest {
        id: Some(837029),
        origin_currency: Some("USD".to_string()),
        origin_amount: Some(6799),
        selection_infos: Vec::new(),
    }
    .encode(&mut data)
    .unwrap();
    let request = ClientPacket {
        sequence: 1,
        cmd_id: CmdId::NewOrderCmd as i16,
        up_tag: 53,
        data,
    };

    crate::handlers::store::on_new_order_at(&mut ctx, request, ACTIVE_TIME_MS)
        .await
        .unwrap();
    let packets = std::iter::from_fn(|| outbound_packets.try_recv().ok()).collect::<Vec<_>>();
    let cmd_ids = packets
        .iter()
        .map(|packet| match packet {
            CommandPacket::Reply { cmd_id, .. } | CommandPacket::Push { cmd_id, .. } => *cmd_id,
        })
        .collect::<Vec<_>>();
    assert_eq!(cmd_ids[0], CmdId::NewOrderCmd);

    let update_index = cmd_ids
        .iter()
        .position(|cmd_id| *cmd_id == CmdId::Act236UpdateInfoPushCmd)
        .unwrap();
    assert_eq!(cmd_ids[update_index - 1], CmdId::MaterialChangePushCmd);
    assert_eq!(cmd_ids[update_index + 1], CmdId::UpdateRedDotPushCmd);
    assert_eq!(cmd_ids[update_index + 2], CmdId::OrderCompletePushCmd);
    assert_eq!(cmd_ids[update_index + 3], CmdId::StatInfoPushCmd);

    let CommandPacket::Push { body, .. } = &packets[update_index - 1] else {
        panic!("material change was not a push");
    };
    assert_eq!(
        MaterialChangePush::decode(&**body).unwrap().get_approach,
        Some(crate::types::material_get_approach::MaterialGetApproach::Charge.id())
    );

    let CommandPacket::Push { body, .. } = &packets[update_index] else {
        panic!("Act236 update was not a push");
    };
    assert_eq!(
        Act236UpdateInfoPush::decode(&**body).unwrap().info,
        Some(Act236Info {
            activity_id: Some(activity_id),
            score: Some(4880),
            gain_reward_ids: vec![1],
        })
    );

    let CommandPacket::Push { body, .. } = &packets[update_index + 1] else {
        panic!("Act236 red dots were not a push");
    };
    let red_dots = UpdateRedDotPush::decode(&**body).unwrap();
    assert_eq!(red_dots.red_dot_infos.len(), 1);
    let group = &red_dots.red_dot_infos[0];
    assert_eq!(
        group.define_id,
        configs::get().activity.get(activity_id).unwrap().red_dot_id
    );
    assert_eq!(group.replace_all, Some(true));
    assert_eq!(
        group
            .infos
            .iter()
            .map(|info| (info.id, info.value, info.time))
            .collect::<Vec<_>>(),
        vec![
            (2, 1, Some(0)),
            (3, 1, Some(0)),
            (4, 1, Some(0)),
            (5, 1, Some(0)),
            (6, 1, Some(0)),
        ]
    );

    assert_eq!(
        database::db::game::activity236::get_state(state.db, player_id, activity_id)
            .await
            .unwrap(),
        database::db::game::activity236::Activity236State {
            score: 4880,
            gain_reward_ids: vec![1],
        }
    );
}
