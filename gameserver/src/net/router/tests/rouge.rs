use super::*;
use sonettobuf::{
    EnterRouge2SelectDifficultyReply, EnterRouge2SelectDifficultyRequest, Rouge2AlchemyReply,
    Rouge2AlchemyRequest, Rouge2Attr, Rouge2AttrInfo, Rouge2AttrUpdatePush, Rouge2Bag,
    Rouge2BagInfo, Rouge2CurAlchemyInfo, Rouge2GameCurAlchemyInfo, Rouge2Info, Rouge2LeaderInfo,
    Rouge2UpdateGeniusPointPush, UpdateAchievementPush,
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

#[tokio::test]
async fn difficulty_command_emits_attribute_push_before_reply() {
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
    let player_id = 538;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'rouge2-difficulty-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    database::db::game::rouge::get_or_create_rouge2_outside(&pool, player_id, configs::get())
        .await
        .unwrap();
    sqlx::query(
        "UPDATE user_rouge2_state
         SET game_num = 1, alchemy_formula = 103, alchemy_main_effect = 190047,
             alchemy_sub_effects = '[190901]'
         WHERE user_id = ?",
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
    EnterRouge2SelectDifficultyRequest {
        difficulty: Some(1),
    }
    .encode(&mut data)
    .unwrap();

    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::EnterRouge2SelectDifficultyCmd as i16,
            up_tag: 56,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();

    let CommandPacket::Push {
        cmd_id: CmdId::Rouge2AttrUpdatePushCmd,
        body,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Rouge2 difficulty did not emit attributes first");
    };
    assert_eq!(
        Rouge2AttrUpdatePush::decode(&*body)
            .unwrap()
            .updates
            .into_iter()
            .map(|attribute| (attribute.attr_id.unwrap(), attribute.final_val.unwrap()))
            .collect::<Vec<_>>(),
        vec![
            (320, 1),
            (321, 1000),
            (326, 10000),
            (202, 99),
            (204, 99),
            (205, 3),
            (310, 500),
            (311, 100),
            (312, 100),
            (313, 50),
            (314, 50),
            (315, 100),
            (316, 500),
            (317, 3000),
            (318, 50),
            (319, 100),
        ]
    );

    let CommandPacket::Reply {
        cmd_id: CmdId::EnterRouge2SelectDifficultyCmd,
        body,
        result_code: 0,
        up_tag: 56,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Rouge2 difficulty did not finish with its reply");
    };
    let info = EnterRouge2SelectDifficultyReply::decode(&*body)
        .unwrap()
        .rouge2_info
        .unwrap();
    assert_eq!(
        info,
        Rouge2Info {
            state: Some(1),
            difficulty: Some(1),
            coin: Some(0),
            map_info: None,
            bag_info: Some(Rouge2BagInfo {
                bags: [1, 3, 4]
                    .into_iter()
                    .map(|bag_type| Rouge2Bag {
                        bag_type: Some(bag_type),
                        items: Vec::new(),
                    })
                    .collect(),
            }),
            end_id: Some(0),
            game_num: Some(2),
            leader_info: Some(Rouge2LeaderInfo {
                main_career_id: Some(0),
                career_id: Some(0),
                revival_coin: Some(0),
                equip_active_skill: vec![0, 0, 0],
                consume_attr_num: Some(0),
                summon_talent_id: Vec::new(),
                summon_talent_point: Some(0),
                system_team_id: Some(0),
                attr_drop_str: Some(String::new()),
            }),
            attr_info: Some(Rouge2AttrInfo {
                attr: [
                    (513, 0),
                    (514, 0),
                    (131, 0),
                    (132, 0),
                    (133, 0),
                    (141, 0),
                    (142, 0),
                    (143, 0),
                    (401, 0),
                    (402, 0),
                    (403, 0),
                    (404, 0),
                    (405, 0),
                    (406, 0),
                    (407, 0),
                    (408, 0),
                    (409, 0),
                    (410, 0),
                    (415, 0),
                    (421, 0),
                    (301, 0),
                    (310, 500),
                    (311, 100),
                    (312, 100),
                    (313, 50),
                    (314, 50),
                    (315, 100),
                    (316, 500),
                    (317, 3000),
                    (318, 50),
                    (319, 100),
                    (320, 1),
                    (321, 1000),
                    (322, 0),
                    (323, 0),
                    (324, 0),
                    (325, 0),
                    (326, 10000),
                    (327, 0),
                    (329, 0),
                    (202, 99),
                    (330, 0),
                    (331, 0),
                    (204, 99),
                    (332, 0),
                    (205, 3),
                    (333, 0),
                    (334, 0),
                    (601, 0),
                    (611, 0),
                    (612, 0),
                    (101, 0),
                    (102, 0),
                    (103, 0),
                    (104, 0),
                    (105, 0),
                    (106, 0),
                    (501, 0),
                    (121, 0),
                    (122, 0),
                    (123, 0),
                ]
                .into_iter()
                .map(|(attr_id, final_val)| Rouge2Attr {
                    attr_id: Some(attr_id),
                    final_val: Some(final_val),
                })
                .collect(),
            }),
            alchemy_info: Some(Rouge2GameCurAlchemyInfo {
                formula: Some(103),
                main_effect: Some(190047),
                sub_effect: vec![190901],
            }),
        }
    );
    assert!(packets.try_recv().is_err());
}
