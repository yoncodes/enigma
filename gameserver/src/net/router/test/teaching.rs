use super::*;

#[tokio::test]
async fn teaching_info_command_reaches_handler_and_decodes_reply() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (513, 'teaching-route', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(2);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(513, PlayerState::new(513, 0)));

    let mut data = Vec::new();
    TeachingGetInfoRequest::default().encode(&mut data).unwrap();
    let request = ClientPacket {
        sequence: 1,
        cmd_id: CmdId::TeachingGetInfoCmd as i16,
        up_tag: 9,
        data,
    }
    .encode();

    dispatch_command(&mut ctx, request).await.unwrap();

    let CommandPacket::Reply {
        cmd_id: CmdId::TeachingGetInfoCmd,
        body,
        up_tag: 9,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Teaching information request did not reach its handler");
    };
    let reply = TeachingGetInfoReply::decode(&*body).unwrap();
    let info = reply.teaching_info.unwrap();
    assert!(info.teachinges.is_empty());
    assert!(info.pass_episodes.is_empty());
    assert!(packets.try_recv().is_err());
}

#[tokio::test]
async fn teaching_bonus_command_routes_and_preserves_committed_packet_order() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 514;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'teaching-bonus-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    let teaching_id = configs::get().teaching.iter().next().unwrap().id;
    complete_teaching(&pool, player_id, teaching_id).await;

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(8);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    let mut data = Vec::new();
    TeachingGetBonusRequest {
        teaching_ids: vec![teaching_id],
    }
    .encode(&mut data)
    .unwrap();
    let request = ClientPacket {
        sequence: 1,
        cmd_id: CmdId::TeachingGetBonusCmd as i16,
        up_tag: 10,
        data,
    }
    .encode();

    dispatch_command(&mut ctx, request).await.unwrap();

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Teaching bonus did not emit a currency snapshot push");
    };
    assert_eq!(cmd_id, CmdId::CurrencyChangePushCmd);
    let currency = CurrencyChangePush::decode(&*body).unwrap();
    assert_eq!(currency.change_currency.len(), 1);
    assert_eq!(currency.change_currency[0].currency_id, Some(21));
    assert_eq!(currency.change_currency[0].quantity, Some(30));

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Teaching bonus did not emit a material delta push");
    };
    assert_eq!(cmd_id, CmdId::MaterialChangePushCmd);
    let material = MaterialChangePush::decode(&*body).unwrap();
    assert_eq!(material.get_approach, Some(170));
    assert_eq!(material.data_list.len(), 1);
    assert_eq!(material.data_list[0].materil_type, Some(2));
    assert_eq!(material.data_list[0].materil_id, Some(21));
    assert_eq!(material.data_list[0].quantity, Some(30));

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Teaching bonus did not emit a red-dot clear push");
    };
    assert_eq!(cmd_id, CmdId::UpdateRedDotPushCmd);
    let red_dot = UpdateRedDotPush::decode(&*body).unwrap();
    assert_eq!(red_dot.red_dot_infos.len(), 1);
    assert_eq!(
        red_dot.red_dot_infos[0].define_id,
        crate::types::red_dot_id::RedDotId::TeachingSystem.id()
    );
    assert_eq!(red_dot.red_dot_infos[0].replace_all, Some(true));
    assert_eq!(red_dot.red_dot_infos[0].infos.len(), 1);
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
        panic!("Teaching bonus did not emit its reply");
    };
    assert_eq!(cmd_id, CmdId::TeachingGetBonusCmd);
    assert_eq!(result_code, 0);
    assert_eq!(up_tag, 10);
    let reply = TeachingGetBonusReply::decode(&*body).unwrap();
    assert_eq!(reply.teachinges.len(), 1);
    assert_eq!(reply.teachinges[0].teaching_id, Some(teaching_id));
    assert_eq!(reply.teachinges[0].status, Some(2));
    assert!(packets.try_recv().is_err());

    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM user_teaching_bonus_claims
             WHERE user_id = ? AND teaching_id = ?",
        )
        .bind(player_id)
        .bind(teaching_id)
        .fetch_one(state.db)
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
async fn teaching_bonus_empty_and_multi_requests_are_rejected_before_mutation() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 515;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'teaching-bonus-invalid', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    let teaching_id = configs::get().teaching.iter().next().unwrap().id;
    complete_teaching(&pool, player_id, teaching_id).await;

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(8);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    for teaching_ids in [vec![], vec![teaching_id, teaching_id]] {
        let mut data = Vec::new();
        TeachingGetBonusRequest { teaching_ids }
            .encode(&mut data)
            .unwrap();
        let request = ClientPacket {
            sequence: 1,
            cmd_id: CmdId::TeachingGetBonusCmd as i16,
            up_tag: 11,
            data,
        }
        .encode();

        dispatch_command(&mut ctx, request).await.unwrap();
        assert!(matches!(
            packets.try_recv().unwrap(),
            CommandPacket::Push {
                cmd_id: CmdId::ServerErrorInfoPushCmd,
                ..
            }
        ));
        let CommandPacket::Reply {
            cmd_id,
            result_code,
            ..
        } = packets.try_recv().unwrap()
        else {
            panic!("Invalid teaching bonus request did not receive an error reply");
        };
        assert_eq!(cmd_id, CmdId::TeachingGetBonusCmd);
        assert_eq!(result_code, -4);
    }

    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM user_teaching_bonus_claims WHERE user_id = ?",
        )
        .bind(player_id)
        .fetch_one(state.db)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM currencies WHERE user_id = ?")
            .bind(player_id)
            .fetch_one(state.db)
            .await
            .unwrap(),
        0
    );
}
