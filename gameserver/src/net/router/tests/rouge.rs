use super::*;
use sonettobuf::{
    Rouge2AlchemyReply, Rouge2AlchemyRequest, Rouge2CurAlchemyInfo, Rouge2UpdateGeniusPointPush,
    UpdateAchievementPush,
};
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::test]
async fn alchemy_command_emits_captured_push_order_and_reply() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/excel2json");
    let _ = config::init(data_dir.to_str().unwrap());
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    database::run_migrations(&pool).await.unwrap();
    let player_id = 537;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'rouge2-alchemy-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    database::db::game::rouge::get_or_create_rouge2_outside(&pool, player_id, configs::get())
        .await
        .unwrap();
    for (material_id, num) in [(1008, 1), (1000, 4), (1001, 1), (1101, 1), (1103, 0)] {
        sqlx::query(
            "INSERT INTO user_rouge2_materials
             (user_id, material_id, num, is_seen, updated_at)
             VALUES (?, ?, ?, 1, 0)",
        )
        .bind(player_id)
        .bind(material_id)
        .bind(num)
        .execute(&pool)
        .await
        .unwrap();
    }
    database::db::game::achievements::sync_event(
        &pool,
        player_id,
        logic::task::TaskEvent::Rouge2Alchemy { count: 1 },
    )
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(4);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));
    let mut data = Vec::new();
    Rouge2AlchemyRequest {
        formula: Some(103),
        sub_material: vec![1101],
    }
    .encode(&mut data)
    .unwrap();
    let request = ClientPacket {
        sequence: 1,
        cmd_id: CmdId::Rouge2AlchemyCmd as i16,
        up_tag: 55,
        data,
    }
    .encode();

    dispatch_command(&mut ctx, request).await.unwrap();

    let CommandPacket::Push {
        cmd_id: CmdId::Rouge2UpdateGeniusPointPushCmd,
        body,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Rouge2 alchemy did not update genius points first");
    };
    assert_eq!(
        Rouge2UpdateGeniusPointPush::decode(&*body).unwrap(),
        Rouge2UpdateGeniusPointPush {
            genius_point: Some(1)
        }
    );

    let CommandPacket::Push {
        cmd_id: CmdId::UpdateAchievementPushCmd,
        body,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Rouge2 alchemy did not update achievements second");
    };
    let achievements = UpdateAchievementPush::decode(&*body).unwrap();
    assert_eq!(
        achievements
            .infos
            .iter()
            .map(|info| (info.id.unwrap(), info.progress.unwrap()))
            .collect::<Vec<_>>(),
        vec![(41270901, 2), (41270902, 2)]
    );

    let CommandPacket::Reply {
        cmd_id: CmdId::Rouge2AlchemyCmd,
        body,
        result_code: 0,
        up_tag: 55,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Rouge2 alchemy did not finish with its reply");
    };
    let reply = Rouge2AlchemyReply::decode(&*body).unwrap();
    assert_eq!(
        reply.alchemy_info.as_ref().unwrap().cur_alchemy_info,
        Some(Rouge2CurAlchemyInfo {
            formula: Some(103),
            main_effect: Some(190047),
            sub_effect: vec![190901],
        })
    );
    assert_eq!(reply.sp_event_num, Some(0));
    assert!(packets.try_recv().is_err());
}
