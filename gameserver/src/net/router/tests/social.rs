use super::*;

#[tokio::test]
async fn investigate_commands_reach_handlers_and_persist_links() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 5615;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'investigate-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(3);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    let mut data = Vec::new();
    GetInvestigateRequest {}.encode(&mut data).unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::GetInvestigateCmd as i16,
            up_tag: 61,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();

    let CommandPacket::Reply {
        cmd_id: CmdId::GetInvestigateCmd,
        body,
        up_tag: 61,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("investigation information request did not reach its handler");
    };
    let info = GetInvestigateReply::decode(&*body).unwrap().info.unwrap();
    assert_eq!(info.clue_ids, vec![11, 41, 51, 61]);
    assert_eq!(info.intel_box.len(), 6);

    let mut data = Vec::new();
    PutClueRequest {
        id: Some(1),
        clue_id: Some(11),
    }
    .encode(&mut data)
    .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 2,
            cmd_id: CmdId::PutClueCmd as i16,
            up_tag: 62,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();

    let CommandPacket::Reply {
        cmd_id: CmdId::PutClueCmd,
        body,
        up_tag: 62,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("put-clue request did not reach its handler");
    };
    assert_eq!(
        PutClueReply::decode(&*body).unwrap(),
        PutClueReply {
            id: Some(1),
            clue_id: Some(11),
        }
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM user_investigate_clues
             WHERE user_id = ? AND info_id = 1 AND clue_id = 11",
        )
        .bind(player_id)
        .fetch_one(ctx.state.db)
        .await
        .unwrap(),
        1
    );
    assert!(packets.try_recv().is_err());
}

#[tokio::test]
async fn hero_invitation_commands_emit_rewards_tasks_and_reply_in_order() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 5635;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'hero-invitation-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_dungeon_elements
         (user_id, element_id, is_finished, puzzle_progress, puzzle_updated_at)
         VALUES (?, 311104, 1, '', 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(16);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    let mut data = Vec::new();
    GetHeroInvitationInfoRequest::default()
        .encode(&mut data)
        .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::GetHeroInvitationInfoCmd as i16,
            up_tag: 71,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();

    let CommandPacket::Reply { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("hero invitation information request did not reach its handler");
    };
    assert_eq!(cmd_id, CmdId::GetHeroInvitationInfoCmd);
    let info = GetHeroInvitationInfoReply::decode(&*body)
        .unwrap()
        .info
        .unwrap();
    assert_eq!(info.opened_invite, vec![1, 5]);
    assert!(info.gain_reward.is_empty());
    assert_eq!(info.final_reward, Some(false));

    let mut data = Vec::new();
    GainInviteRewardRequest { id: Some(4) }
        .encode(&mut data)
        .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 2,
            cmd_id: CmdId::GainInviteRewardCmd as i16,
            up_tag: 72,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("hero invitation claim did not emit a currency push");
    };
    assert_eq!(cmd_id, CmdId::CurrencyChangePushCmd);
    assert_eq!(
        CurrencyChangePush::decode(&*body).unwrap().change_currency[0].quantity,
        Some(20_000)
    );
    let CommandPacket::Push { cmd_id, .. } = packets.try_recv().unwrap() else {
        panic!("hero invitation claim did not emit a material push");
    };
    assert_eq!(cmd_id, CmdId::MaterialChangePushCmd);
    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("hero invitation claim did not emit a task update");
    };
    assert_eq!(cmd_id, CmdId::UpdateTaskPushCmd);
    let task_push = UpdateTaskPush::decode(&*body).unwrap();
    assert_eq!(
        task_push
            .task_info
            .iter()
            .find(|task| task.id == 110918)
            .map(|task| task.progress),
        Some(1)
    );
    let CommandPacket::Reply {
        cmd_id,
        body,
        up_tag,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("hero invitation claim did not emit its reply");
    };
    assert_eq!(cmd_id, CmdId::GainInviteRewardCmd);
    assert_eq!(up_tag, 72);
    assert_eq!(
        GainInviteRewardReply::decode(&*body)
            .unwrap()
            .info
            .unwrap()
            .gain_reward,
        vec![4]
    );
    assert!(packets.try_recv().is_err());

    let mut data = Vec::new();
    GainInviteRewardRequest { id: Some(4) }
        .encode(&mut data)
        .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 3,
            cmd_id: CmdId::GainInviteRewardCmd as i16,
            up_tag: 73,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();
    let CommandPacket::Reply { cmd_id, .. } = packets.try_recv().unwrap() else {
        panic!("idempotent hero invitation claim did not emit its reply");
    };
    assert_eq!(cmd_id, CmdId::GainInviteRewardCmd);
    assert!(packets.try_recv().is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT progress FROM user_tasks
             WHERE user_id = ? AND type_id = 11 AND task_id = 110918",
        )
        .bind(player_id)
        .fetch_one(state.db)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT quantity FROM currencies WHERE user_id = ? AND currency_id = 3",
        )
        .bind(player_id)
        .fetch_one(state.db)
        .await
        .unwrap(),
        20_000
    );
}
