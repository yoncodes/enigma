use super::*;

#[tokio::test]
async fn act233_info_command_reaches_handler_and_returns_configured_state() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (512, 'act233-route', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let pass = configs::get().activity233_bp.iter().next().unwrap();
    let expected_tasks = configs::get()
        .activity233_task
        .iter()
        .filter(|task| {
            task.activity_id == pass.activity_id
                && task.bp_id == pass.bp_id
                && task.is_online != 0
        })
        .count();
    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(2);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(512, PlayerState::new(512, 0)));

    let mut data = Vec::new();
    GetAct233BpInfoRequest {
        get_task: Some(true),
        activity_id: Some(pass.activity_id),
    }
    .encode(&mut data)
    .unwrap();
    let request = ClientPacket {
        sequence: 1,
        cmd_id: CmdId::GetAct233BpInfoCmd as i16,
        up_tag: 7,
        data,
    }
    .encode();

    dispatch_command(&mut ctx, request).await.unwrap();

    let CommandPacket::Reply {
        cmd_id: CmdId::GetAct233BpInfoCmd,
        body,
        up_tag: 7,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Act233 information request did not reach its handler");
    };
    let reply = GetAct233BpInfoReply::decode(&*body).unwrap();
    assert_eq!(reply.activity_id, Some(pass.activity_id));
    assert_eq!(reply.bp_id, Some(pass.bp_id));
    assert_eq!(reply.task_info.len(), expected_tasks);
    assert!(packets.try_recv().is_err());
}

#[tokio::test]
async fn act233_finish_task_emits_task_and_absolute_score_pushes() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (513, 'act233-finish-route', 0, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_tasks
         (user_id, type_id, task_id, progress, has_finished, finish_count, activity_id)
         VALUES (513, 79, 790001, 1, 1, 0, 13716)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_act233_bp_state (user_id, activity_id, bp_id, score)
         VALUES (513, 13716, 1, 100)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(16);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(
        513,
        PlayerState::new(513, ::common::time::ServerTime::now_ms()),
    ));

    let mut data = Vec::new();
    FinishTaskRequest { id: 790001 }.encode(&mut data).unwrap();
    let request = ClientPacket {
        sequence: 1,
        cmd_id: CmdId::FinishTaskCmd as i16,
        up_tag: 8,
        data,
    }
    .encode();

    dispatch_command(&mut ctx, request).await.unwrap();

    let mut task_update = None;
    let mut score_update = None;
    let mut reply = None;
    while let Ok(packet) = packets.try_recv() {
        match packet {
            CommandPacket::Push {
                cmd_id: CmdId::UpdateTaskPushCmd,
                body,
                ..
            } => {
                task_update = Some(UpdateTaskPush::decode(&*body).unwrap());
            }
            CommandPacket::Push {
                cmd_id: CmdId::Act233BpScoreUpdatePushCmd,
                body,
                ..
            } => {
                score_update = Some(Act233BpScoreUpdatePush::decode(&*body).unwrap());
            }
            CommandPacket::Reply {
                cmd_id: CmdId::FinishTaskCmd,
                body,
                ..
            } => {
                reply = Some(FinishTaskReply::decode(&*body).unwrap());
            }
            _ => {}
        }
    }

    let task_update = task_update.expect("Act233 task update push");
    assert_eq!(task_update.task_info[0].id, 790001);
    assert_eq!(task_update.task_info[0].finish_count, Some(1));
    assert_eq!(
        score_update.expect("Act233 score update push"),
        Act233BpScoreUpdatePush {
            activity_id: Some(13716),
            bp_id: Some(1),
            score: Some(200),
        }
    );
    assert_eq!(reply.unwrap().finish_count, Some(1));
}

#[tokio::test]
async fn act233_bonus_command_routes_after_committed_reward_and_red_dot_pushes() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 516;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'act233-bonus-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    let pass = configs::get().activity233_bp.iter().next().unwrap();
    sqlx::query(
        "INSERT INTO user_act233_bp_state
         (user_id, activity_id, bp_id, score)
         VALUES (?, ?, ?, ?)",
    )
    .bind(player_id)
    .bind(pass.activity_id)
    .bind(pass.bp_id)
    .bind(pass.exp_level_up * 3)
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(8);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    let mut data = Vec::new();
    GetAct233BpBonusRequest {
        activity_id: Some(pass.activity_id),
        level: Some(0),
        pay_bonus: Some(false),
    }
    .encode(&mut data)
    .unwrap();
    let request = ClientPacket {
        sequence: 1,
        cmd_id: CmdId::GetAct233BpBonusCmd as i16,
        up_tag: 12,
        data,
    }
    .encode();

    dispatch_command(&mut ctx, request).await.unwrap();

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Act233 bonus did not emit a currency snapshot push");
    };
    assert_eq!(cmd_id, CmdId::CurrencyChangePushCmd);
    assert!(
        !CurrencyChangePush::decode(&*body)
            .unwrap()
            .change_currency
            .is_empty()
    );

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Act233 bonus did not emit an item snapshot push");
    };
    assert_eq!(cmd_id, CmdId::ItemChangePushCmd);
    assert!(!ItemChangePush::decode(&*body).unwrap().items.is_empty());

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Act233 bonus did not emit the trade red-dot projection");
    };
    assert_eq!(cmd_id, CmdId::UpdateRedDotPushCmd);
    let trade = UpdateRedDotPush::decode(&*body).unwrap();
    assert_eq!(
        trade.red_dot_infos[0].define_id,
        crate::types::red_dot_id::RedDotId::TradeOrderFulfillable.id()
    );

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Act233 bonus did not emit a material delta push");
    };
    assert_eq!(cmd_id, CmdId::MaterialChangePushCmd);
    let material = MaterialChangePush::decode(&*body).unwrap();
    assert_eq!(
        material.get_approach,
        Some(crate::types::material_get_approach::MaterialGetApproach::ActBp.id())
    );
    assert!(!material.data_list.is_empty());

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Act233 bonus did not emit its red-dot projection");
    };
    assert_eq!(cmd_id, CmdId::UpdateRedDotPushCmd);
    let red_dots = UpdateRedDotPush::decode(&*body).unwrap();
    assert_eq!(red_dots.red_dot_infos.len(), 2);
    assert_eq!(
        red_dots
            .red_dot_infos
            .iter()
            .map(|group| (group.define_id, group.infos[0].value))
            .collect::<Vec<_>>(),
        vec![
            (
                crate::types::red_dot_id::RedDotId::V3a7Anniversary3ActBpSubTask.id(),
                0,
            ),
            (
                crate::types::red_dot_id::RedDotId::V3a7Anniversary3ActBpBonus.id(),
                0,
            ),
        ]
    );
    assert!(
        red_dots
            .red_dot_infos
            .iter()
            .all(|group| group.replace_all == Some(true))
    );

    let CommandPacket::Reply {
        cmd_id,
        body,
        result_code,
        up_tag,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Act233 bonus did not emit its reply");
    };
    assert_eq!(cmd_id, CmdId::GetAct233BpBonusCmd);
    assert_eq!(result_code, 0);
    assert_eq!(up_tag, 12);
    let reply = GetAct233BpBonusReply::decode(&*body).unwrap();
    assert_eq!(reply.activity_id, Some(pass.activity_id));
    assert_eq!(reply.bp_id, Some(pass.bp_id));
    assert_eq!(reply.score_bonus_info.len(), 3);
    assert_eq!(
        reply
            .score_bonus_info
            .iter()
            .map(|info| (info.level, info.has_getfree_bonus, info.has_get_pay_bonus))
            .collect::<Vec<_>>(),
        vec![
            (Some(1), Some(true), None),
            (Some(2), Some(true), None),
            (Some(3), Some(true), None),
        ]
    );
    assert!(packets.try_recv().is_err());

    let claimed: String = sqlx::query_scalar(
        "SELECT has_get_free_bonus FROM user_act233_bp_state
         WHERE user_id = ? AND activity_id = ? AND bp_id = ?",
    )
    .bind(player_id)
    .bind(pass.activity_id)
    .bind(pass.bp_id)
    .fetch_one(state.db)
    .await
    .unwrap();
    assert_eq!(claimed, "[1,2,3]");
}
