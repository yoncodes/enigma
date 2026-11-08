use super::*;
use sonettobuf::{
    CurrencyChangePush, EnterRouge2SelectDifficultyReply, EnterRouge2SelectDifficultyRequest,
    HeroGainPush, HeroUpdatePush, ItemChangePush, MaterialChangePush, Rouge2ActiveGeniusReply,
    Rouge2ActiveGeniusRequest, Rouge2AlchemyReply, Rouge2AlchemyRequest, Rouge2Attr,
    Rouge2AttrInfo, Rouge2AttrUpdatePush, Rouge2Bag, Rouge2BagInfo, Rouge2CurAlchemyInfo,
    Rouge2GameCurAlchemyInfo, Rouge2Info, Rouge2LeaderInfo, Rouge2RewardReply, Rouge2RewardRequest,
    Rouge2SetSystemIdReply, Rouge2SetSystemIdRequest, Rouge2UpdateGeniusPointPush,
    UpdateAchievementPush, UpdateRedDotPush,
};
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::test]
async fn active_genius_command_updates_points_before_reply() {
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
    let player_id = 539;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'rouge2-genius-route', 0, 0)",
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
         SET genius_point = 1, genius_ids = '[10009]'
         WHERE user_id = ?",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(2);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));
    let mut data = Vec::new();
    Rouge2ActiveGeniusRequest {
        genius_id: Some(10010),
    }
    .encode(&mut data)
    .unwrap();

    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::Rouge2ActiveGeniusCmd as i16,
            up_tag: 57,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();

    let CommandPacket::Push {
        cmd_id: CmdId::Rouge2UpdateGeniusPointPushCmd,
        body,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Rouge2 talent activation did not update genius points first");
    };
    assert_eq!(
        Rouge2UpdateGeniusPointPush::decode(&*body).unwrap(),
        Rouge2UpdateGeniusPointPush {
            genius_point: Some(0)
        }
    );

    let CommandPacket::Reply {
        cmd_id: CmdId::Rouge2ActiveGeniusCmd,
        body,
        result_code: 0,
        up_tag: 57,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Rouge2 talent activation did not finish with its reply");
    };
    assert_eq!(
        Rouge2ActiveGeniusReply::decode(&*body).unwrap(),
        Rouge2ActiveGeniusReply {
            genius_id: Some(10010),
            genius_ids: vec![10009, 10010],
        }
    );
    assert!(packets.try_recv().is_err());
}

#[tokio::test]
async fn reward_command_emits_captured_purchase_push_order() {
    crate::net::app::init_test_server_config();
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
    let player_id = 540;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'rouge2-reward-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    database::db::game::rouge::get_or_create_rouge2_outside(&pool, player_id, configs::get())
        .await
        .unwrap();
    database::db::game::currencies::add_currency(&pool, player_id, 3201, 8960)
        .await
        .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(5);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));
    let mut data = Vec::new();
    Rouge2RewardRequest {
        id: Some(3002),
        num: Some(4),
    }
    .encode(&mut data)
    .unwrap();

    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::Rouge2RewardCmd as i16,
            up_tag: 58,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Rouge2 reward did not emit currency first");
    };
    assert_eq!(cmd_id, CmdId::CurrencyChangePushCmd);
    let currency = CurrencyChangePush::decode(&*body).unwrap();
    assert_eq!(currency.change_currency[0].currency_id, Some(3201));
    assert_eq!(currency.change_currency[0].quantity, Some(960));

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Rouge2 reward did not emit item state second");
    };
    assert_eq!(cmd_id, CmdId::ItemChangePushCmd);
    let items = ItemChangePush::decode(&*body).unwrap();
    let item = &items.items[0];
    assert_eq!((item.item_id, item.quantity), (Some(133136), Some(4)));

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Rouge2 reward did not emit the trade-order red dot third");
    };
    assert_eq!(cmd_id, CmdId::UpdateRedDotPushCmd);
    let red_dot = UpdateRedDotPush::decode(&*body).unwrap();
    assert_eq!(
        red_dot.red_dot_infos[0].define_id,
        crate::types::red_dot_id::RedDotId::TradeOrderFulfillable.id()
    );

    let CommandPacket::Push { cmd_id, body, .. } = packets.try_recv().unwrap() else {
        panic!("Rouge2 reward did not emit material state fourth");
    };
    assert_eq!(cmd_id, CmdId::MaterialChangePushCmd);
    let material = MaterialChangePush::decode(&*body).unwrap();
    assert_eq!(
        material.get_approach,
        Some(crate::types::material_get_approach::MaterialGetApproach::Activity.id())
    );
    assert_eq!(
        material
            .data_list
            .iter()
            .map(|entry| (entry.materil_type, entry.materil_id, entry.quantity))
            .collect::<Vec<_>>(),
        vec![(Some(1), Some(133136), Some(4))]
    );

    let CommandPacket::Reply {
        cmd_id: CmdId::Rouge2RewardCmd,
        body,
        result_code: 0,
        up_tag: 58,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Rouge2 reward did not finish with its reply");
    };
    let reply = Rouge2RewardReply::decode(&*body).unwrap();
    assert_eq!(reply.num, Some(4));
    assert_eq!(reply.goods_info.unwrap().buy_count, Some(4));
    assert!(packets.try_recv().is_err());
}

#[tokio::test]
async fn set_system_id_command_echoes_the_persisted_selection() {
    crate::net::app::init_test_server_config();
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
    let player_id = 542;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'rouge2-system-route', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    database::db::game::rouge::get_or_create_rouge2_outside(&pool, player_id, configs::get())
        .await
        .unwrap();
    sqlx::query("UPDATE user_rouge2_state SET state = 2 WHERE user_id = ?")
        .bind(player_id)
        .execute(&pool)
        .await
        .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(1);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));
    let mut data = Vec::new();
    Rouge2SetSystemIdRequest {
        system_id: Some(131),
    }
    .encode(&mut data)
    .unwrap();

    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::Rouge2SetSystemIdCmd as i16,
            up_tag: 61,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();

    let CommandPacket::Reply {
        cmd_id: CmdId::Rouge2SetSystemIdCmd,
        body,
        result_code: 0,
        up_tag: 61,
        ..
    } = packets.try_recv().unwrap()
    else {
        panic!("Rouge2 system selection did not return its captured reply");
    };
    assert_eq!(
        Rouge2SetSystemIdReply::decode(&*body).unwrap().system_id,
        Some(131)
    );
    assert!(packets.try_recv().is_err());
}

#[tokio::test]
async fn reward_command_emits_captured_equip_and_hero_pushes() {
    crate::net::app::init_test_server_config();
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
    let player_id = 541;
    sqlx::query(
        "INSERT INTO users (id, username, created_at, updated_at)
         VALUES (?, 'rouge2-reward-projections', 0, 0)",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();
    database::db::game::player_infos::create_player_info(&pool, player_id, 0)
        .await
        .unwrap();
    database::db::game::rouge::get_or_create_rouge2_outside(&pool, player_id, configs::get())
        .await
        .unwrap();
    let currency_id = ::common::config().rouge2.store_currency_id;
    database::db::game::currencies::add_currency(&pool, player_id, currency_id, 18_000)
        .await
        .unwrap();

    let state = Box::leak(Box::new(AppState::new(pool, configs::get())));
    let (outbound, mut packets) = mpsc::channel(16);
    let mut ctx = ConnectionContext::new(outbound, state);
    ctx.player = Some(Player::new(player_id, PlayerState::new(player_id, 0)));

    let mut data = Vec::new();
    Rouge2RewardRequest {
        id: Some(3000),
        num: Some(1),
    }
    .encode(&mut data)
    .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 1,
            cmd_id: CmdId::Rouge2RewardCmd as i16,
            up_tag: 59,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();
    let equip_packets = (0..4)
        .map(|_| packets.try_recv().unwrap())
        .collect::<Vec<_>>();
    assert!(matches!(
        &equip_packets[0],
        CommandPacket::Push {
            cmd_id: CmdId::CurrencyChangePushCmd,
            ..
        }
    ));
    assert!(matches!(
        &equip_packets[1],
        CommandPacket::Push {
            cmd_id: CmdId::EquipUpdatePushCmd,
            ..
        }
    ));
    let CommandPacket::Push { body, .. } = &equip_packets[2] else {
        panic!("Rouge2 equipment reward did not emit material state third");
    };
    let material = MaterialChangePush::decode(&**body).unwrap();
    assert_eq!(
        material
            .data_list
            .iter()
            .map(|entry| (entry.materil_type, entry.materil_id, entry.quantity))
            .collect::<Vec<_>>(),
        vec![(Some(9), Some(1570), Some(1))]
    );
    assert!(matches!(
        &equip_packets[3],
        CommandPacket::Reply {
            cmd_id: CmdId::Rouge2RewardCmd,
            result_code: 0,
            up_tag: 59,
            ..
        }
    ));

    let mut data = Vec::new();
    Rouge2RewardRequest {
        id: Some(3001),
        num: Some(1),
    }
    .encode(&mut data)
    .unwrap();
    dispatch_command(
        &mut ctx,
        ClientPacket {
            sequence: 2,
            cmd_id: CmdId::Rouge2RewardCmd as i16,
            up_tag: 60,
            data,
        }
        .encode(),
    )
    .await
    .unwrap();
    let mut hero_packets = Vec::new();
    while let Ok(packet) = packets.try_recv() {
        hero_packets.push(packet);
    }
    let expected = [
        CmdId::CurrencyChangePushCmd,
        CmdId::HeroHeroUpdatePushCmd,
        CmdId::PlayerInfoPushCmd,
        CmdId::PlayerCardInfoPushCmd,
        CmdId::PlayerCardInfoPushCmd,
        CmdId::HeroHeroUpdatePushCmd,
        CmdId::HeroGainPushCmd,
        CmdId::MaterialChangePushCmd,
        CmdId::Rouge2RewardCmd,
    ];
    assert_eq!(
        hero_packets
            .iter()
            .map(|packet| match packet {
                CommandPacket::Push { cmd_id, .. } | CommandPacket::Reply { cmd_id, .. } => *cmd_id,
            })
            .collect::<Vec<_>>(),
        expected
    );
    let CommandPacket::Push { body, .. } = &hero_packets[1] else {
        panic!("Rouge2 hero reward did not emit the initial hero state");
    };
    let initial = HeroUpdatePush::decode(&**body).unwrap();
    let CommandPacket::Push { body, .. } = &hero_packets[5] else {
        panic!("Rouge2 hero reward did not emit the voice-unlocked hero state");
    };
    let final_state = HeroUpdatePush::decode(&**body).unwrap();
    assert_eq!(initial.hero_updates[0].voice, Vec::<i32>::new());
    assert_eq!(final_state.hero_updates[0].voice, vec![1_313_601]);
    let mut expected_initial = final_state.hero_updates[0].clone();
    expected_initial.voice.clear();
    assert_eq!(initial.hero_updates[0], expected_initial);
    let CommandPacket::Push { body, .. } = &hero_packets[6] else {
        panic!("Rouge2 hero reward did not emit HeroGainPush");
    };
    let gain = HeroGainPush::decode(&**body).unwrap();
    assert_eq!(
        (gain.hero_id, gain.is_first, gain.duplicate_count),
        (Some(3136), Some(true), Some(0))
    );
    assert_eq!(
        gain.items
            .iter()
            .map(|entry| (entry.materil_type, entry.materil_id, entry.quantity))
            .collect::<Vec<_>>(),
        vec![(Some(4), Some(3136), Some(1))]
    );
    let CommandPacket::Push { body, .. } = &hero_packets[7] else {
        panic!("Rouge2 hero reward did not emit material state");
    };
    let material = MaterialChangePush::decode(&**body).unwrap();
    assert_eq!(
        material
            .data_list
            .iter()
            .map(|entry| (entry.materil_type, entry.materil_id, entry.quantity))
            .collect::<Vec<_>>(),
        vec![(Some(4), Some(3136), Some(1))]
    );
    assert!(packets.try_recv().is_err());
}

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
