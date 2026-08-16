use super::*;

#[tokio::test]
async fn act239_commands_route_and_emit_the_captured_claim_sequence() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 541;
    let row = configs::get()
        .activity239
        .iter()
        .find(|row| row.id == 3)
        .unwrap();
    let activity_id = row.activity_id;
    let reward_id = row.id;
    let red_dot_id = configs::get().activity.get(activity_id).unwrap().red_dot_id;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'act239-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    for entry_id in configs::get()
        .activity239
        .iter()
        .filter(|row| row.activity_id == activity_id && row.id != reward_id)
        .map(|row| row.id)
    {
        database::db::game::activity_state::set(
            &pool,
            player_id,
            activity_id,
            database::db::game::activity_state::ActivityStateSet {
                kind: database::db::game::activity_state::ActivityStateKind::Act239Bonus,
                entry_id,
                state: 2,
                progress: 0,
                ext: "",
            },
        )
        .await
        .unwrap();
    }

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(8);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    let mut data = Vec::new();
    GetAct239InfoRequest {
        activity_id: Some(activity_id),
    }
    .encode(&mut data)
    .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::GetAct239InfoCmd as i16,
            up_tag: 31,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();

    let CommandPacket::Reply {
        cmd_id: CmdId::GetAct239InfoCmd,
        body,
        result_code: 0,
        up_tag: 31,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Activity 239 information request did not reach its handler");
    };
    let info = GetAct239InfoReply::decode(&*body).unwrap();
    assert_eq!(info.activity_id, Some(activity_id));
    assert!(
        info.bonuss
            .iter()
            .any(|bonus| bonus.id == Some(reward_id) && bonus.status == Some(1))
    );

    let mut data = Vec::new();
    Act239BonusRequest {
        activity_id: Some(activity_id),
        id: Some(reward_id),
    }
    .encode(&mut data)
    .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 2,
            cmd_id: CmdId::Act239BonusCmd as i16,
            up_tag: 32,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Activity 239 claim did not emit its currency snapshot first");
    };
    assert_eq!(cmd_id, CmdId::CurrencyChangePushCmd);
    let currency = CurrencyChangePush::decode(&*body).unwrap();
    assert_eq!(currency.change_currency[0].currency_id, Some(2));
    assert_eq!(currency.change_currency[0].quantity, Some(60));

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Activity 239 claim did not emit its material delta");
    };
    assert_eq!(cmd_id, CmdId::MaterialChangePushCmd);
    let material = MaterialChangePush::decode(&*body).unwrap();
    assert_eq!(material.get_approach, Some(177));
    assert_eq!(material.data_list[0].materil_type, Some(2));
    assert_eq!(material.data_list[0].materil_id, Some(2));
    assert_eq!(material.data_list[0].quantity, Some(60));

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Activity 239 claim did not emit its red-dot replacement");
    };
    assert_eq!(cmd_id, CmdId::UpdateRedDotPushCmd);
    let red_dot = UpdateRedDotPush::decode(&*body).unwrap();
    assert_eq!(red_dot.red_dot_infos[0].define_id, red_dot_id);
    assert_eq!(red_dot.red_dot_infos[0].replace_all, Some(true));
    assert_eq!(red_dot.red_dot_infos[0].infos.len(), 1);
    assert_eq!(red_dot.red_dot_infos[0].infos[0].id, 0);
    assert_eq!(red_dot.red_dot_infos[0].infos[0].value, 0);

    let CommandPacket::Reply {
        cmd_id: CmdId::Act239BonusCmd,
        body,
        result_code: 0,
        up_tag: 32,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Activity 239 claim did not emit its reply last");
    };
    let reply = Act239BonusReply::decode(&*body).unwrap();
    assert_eq!(reply.activity_id, Some(activity_id));
    assert!(
        reply
            .bonuss
            .iter()
            .any(|bonus| bonus.id == Some(reward_id) && bonus.status == Some(2))
    );
    assert!(reply.bonuss.iter().all(|bonus| bonus.status == Some(2)));
    assert!(packets.try_recv().is_err());
}

#[tokio::test]
async fn act128_milestone_command_emits_the_captured_reward_sequence() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 515;
    let activity_id = configs::get().latest_open_activity_id(128).unwrap();
    let currency_id = configs::get().activity128_rank_currency_id().unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'act128-milestone-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO currencies (user_id, currency_id, quantity)
         VALUES (?, ?, 700)",
    )
    .bind(player_id)
    .bind(currency_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_activity_state
         (user_id, activity_id, kind, entry_id, state, progress, ext, updated_at)
         VALUES (?, ?, ?, 0, 2, 0, '', 0)",
    )
    .bind(player_id)
    .bind(activity_id)
    .bind(database::db::game::activity_state::ActivityStateKind::Act128Milestone.id())
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(8);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    let mut data = Vec::new();
    Act128GetMilestoneBonusRequest {
        activity_id: Some(activity_id),
    }
    .encode(&mut data)
    .unwrap();
    let request = ClientPacket {
        sequence: 1,
        cmd_id: CmdId::Act128GetMilestoneBonusCmd as i16,
        up_tag: 10,
        data,
    }
    .encode();

    dispatch_command(&mut ctx, request).await.unwrap();

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Act128 milestone did not emit item snapshots first");
    };
    assert_eq!(cmd_id, CmdId::ItemChangePushCmd);
    let items = ItemChangePush::decode(&*body).unwrap().items;
    assert_eq!(
        items
            .iter()
            .map(|item| (item.item_id, item.quantity))
            .collect::<Vec<_>>(),
        vec![(Some(120013), Some(2)), (Some(110404), Some(1))]
    );

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Act128 milestone did not emit the trade-order red dot");
    };
    assert_eq!(cmd_id, CmdId::UpdateRedDotPushCmd);
    let trade = UpdateRedDotPush::decode(&*body).unwrap();
    assert_eq!(
        trade.red_dot_infos[0].define_id,
        crate::types::red_dot_id::RedDotId::TradeOrderFulfillable.id()
    );

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Act128 milestone did not emit material deltas");
    };
    assert_eq!(cmd_id, CmdId::MaterialChangePushCmd);
    let material = MaterialChangePush::decode(&*body).unwrap();
    assert_eq!(
        material.get_approach,
        Some(
            crate::types::material_get_approach::MaterialGetApproach::Act128MilestoneBonus.id()
        )
    );
    assert_eq!(
        material
            .data_list
            .iter()
            .map(|entry| (entry.materil_type, entry.materil_id, entry.quantity))
            .collect::<Vec<_>>(),
        vec![
            (Some(1), Some(120013), Some(2)),
            (Some(1), Some(110404), Some(1))
        ]
    );

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Act128 milestone did not clear its rank red dot");
    };
    assert_eq!(cmd_id, CmdId::UpdateRedDotPushCmd);
    let rank = UpdateRedDotPush::decode(&*body).unwrap();
    assert_eq!(rank.red_dot_infos.len(), 1);
    assert_eq!(
        rank.red_dot_infos[0].define_id,
        crate::types::red_dot_id::RedDotId::BossRushRankBonus.id()
    );
    assert_eq!(rank.red_dot_infos[0].replace_all, Some(true));
    assert_eq!(rank.red_dot_infos[0].infos[0].id, 0);
    assert_eq!(rank.red_dot_infos[0].infos[0].value, 0);

    let CommandPacket::Reply {
        cmd_id,
        body,
        result_code,
        up_tag,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Act128 milestone did not emit its reply last");
    };
    assert_eq!(cmd_id, CmdId::Act128GetMilestoneBonusCmd);
    assert_eq!(result_code, 0);
    assert_eq!(up_tag, 10);
    let reply = Act128GetMilestoneBonusReply::decode(&*body).unwrap();
    assert_eq!(reply.activity_id, Some(activity_id));
    assert_eq!(reply.gain_milestone_level, Some(7));
    assert!(packets.try_recv().is_err());
}
