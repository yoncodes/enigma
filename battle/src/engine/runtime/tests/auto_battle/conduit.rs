use super::*;

#[test]
fn prefix_targets_use_queue_admission_state() {
    let mut sonetto = team_hero(11, 3023, 1_000, [30230111, 30230121]);
    sonetto.attr.as_mut().unwrap().attack = Some(1_000);
    let mut runtime = team_runtime_against(
        vec![team_hero(10, 3145, 1_000, [31450111, 31450121]), sonetto],
        vec![enemy(-1, 1), enemy(-2, 100_000)],
        vec![
            hand_card(11, 30230111, false),
            hand_card(11, 30230111, false),
            hand_card(10, 31446062, false),
        ],
    );
    runtime.round_state.act_point = 3;
    let mut request = AutoRoundRequest {
        opers: vec![
            BeginRoundOper {
                oper_type: Some(CardOpType::PlayCard.id()),
                param1: Some(1),
                to_id: Some(-1),
                ..Default::default()
            };
            2
        ],
        ..Default::default()
    };

    let reply = runtime.plan_auto_round(&request);

    assert_eq!(reply.opers.len(), 1);
    assert_eq!(reply.opers[0].param1, Some(1));
    request.opers.extend(reply.opers);
    runtime
        .advance_round(BeginRoundRequest {
            opers: request.opers,
            devices_opers: reply.devices_opers,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(runtime.managers.hp.current(-1), 0);
    assert!(runtime.managers.hp.current(-2) > 0);
}

#[test]
fn auto_conduit_plan_replays_the_enabling_buff_boundary() {
    for enabled in [false, true] {
        let mut everecho = team_hero(10, 3145, 1_000, [31450111, 31450121]);
        everecho.passive_skill = vec![31450141];
        everecho.attr.as_mut().unwrap().attack = Some(100);
        let mut runtime = team_runtime(
            vec![everecho, team_hero(11, 3023, 500, [30230111, 30230121])],
            vec![hand_card(10, 31446062, false)],
        );
        if enabled {
            grant_buff(&mut runtime, 10, 10, 31450121);
        }

        let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

        if !enabled {
            assert_eq!(reply.devices_opers[0].index, Some(1));
        }
        assert_eq!(runtime.managers.hp.current(11), 500);
        assert_eq!(runtime.managers.buff.has_buff_id(10, 31450121), enabled);
        runtime
            .advance_round(BeginRoundRequest {
                opers: reply.opers,
                devices_opers: reply.devices_opers,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(runtime.managers.hp.current(11) > 500, enabled);
        assert!(!runtime.managers.buff.has_buff_id(10, 31450121));
    }
}

#[test]
fn auto_round_keeps_equal_special_energy_cards_in_hand_order() {
    let mut runtime = team_runtime(
        vec![team_hero(10, 3149, 1_000, [31490111, 31490121])],
        vec![
            hand_card(10, 31490211, false),
            hand_card(10, 31490211, false),
        ],
    );
    runtime.round_state.act_point = 2;

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(
        reply
            .opers
            .iter()
            .map(|operation| operation.param1)
            .collect::<Vec<_>>(),
        vec![Some(1), Some(1)]
    );
}

#[test]
fn auto_round_targets_an_enemy_with_hedones_unlocked_energy_card() {
    let mut hedone = team_hero(10, 3156, 1_000, [31560111, 31560121]);
    hedone.buffs.push(sonettobuf::BuffInfo {
        uid: Some(20),
        buff_id: Some(31560185),
        from_uid: Some(10),
        act_info: vec![sonettobuf::BuffActInfo {
            act_id: Some(1159),
            param: vec![2, 10, 1],
            str_param: Some(String::new()),
        }],
        ..Default::default()
    });
    let free = |skill_id| sonettobuf::CardInfo {
        extra_infos: vec![sonettobuf::CardExtraInfo {
            key: Some(8),
            values: vec!["HedoneCard".to_owned()],
        }],
        ..hand_card(10, skill_id, false)
    };
    let runtime = team_runtime(vec![hedone], vec![free(31446011), free(31560181)]);

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers.len(), 2);
    assert_eq!(reply.opers[0].param1, Some(2));
    assert!(
        reply
            .opers
            .iter()
            .all(|operation| operation.to_id == Some(-1))
    );
    assert_eq!(
        reply
            .devices_opers
            .iter()
            .map(|operation| (operation.uid, operation.index))
            .collect::<Vec<_>>(),
        [(Some(10), Some(1))]
    );

    let continued = runtime
        .try_plan_auto_round(&AutoRoundRequest {
            opers: vec![reply.opers[0].clone()],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(continued.opers.len(), 1);
    assert_eq!(continued.opers[0].param1, Some(1));
    assert_eq!(continued.opers[0].to_id, Some(-1));
}

#[test]
fn queued_hedone_energy_feeds_her_card_driven_group_switch() {
    let mut hedone = team_hero(10, 3156, 1_000, [31560111, 31560121]);
    hedone.buffs.push(sonettobuf::BuffInfo {
        uid: Some(20),
        buff_id: Some(31560185),
        from_uid: Some(10),
        act_info: vec![sonettobuf::BuffActInfo {
            act_id: Some(1159),
            param: vec![2, 10, 1],
            str_param: Some(String::new()),
        }],
        ..Default::default()
    });
    let energy = sonettobuf::CardInfo {
        extra_infos: vec![sonettobuf::CardExtraInfo {
            key: Some(8),
            values: vec!["HedoneCard".to_owned()],
        }],
        ..hand_card(10, 31560181, false)
    };
    let runtime = team_runtime(vec![hedone], vec![hand_card(10, 31560111, false), energy]);
    assert_eq!(runtime.managers.conduit.selected_group(10), Some(1));
    let play = |target_uid| BeginRoundOper {
        oper_type: Some(CardOpType::PlayCard.id()),
        param1: Some(1),
        to_id: Some(target_uid),
        ..Default::default()
    };

    let reply = runtime
        .try_plan_auto_round(&AutoRoundRequest {
            opers: vec![play(-1), play(-1)],
            ..Default::default()
        })
        .unwrap();

    assert!(reply.opers.is_empty());
    assert_eq!(reply.devices_opers[0].index, Some(2));
}

#[test]
fn auto_round_rejects_invalid_hedone_owned_state() {
    let mut hedone = team_hero(10, 3156, 1_000, [31560111, 31560121]);
    hedone.buffs.push(sonettobuf::BuffInfo {
        uid: Some(20),
        buff_id: Some(31560185),
        from_uid: Some(10),
        act_info: vec![sonettobuf::BuffActInfo {
            act_id: Some(1159),
            param: vec![2, 10],
            str_param: Some(String::new()),
        }],
        ..Default::default()
    });
    let runtime = team_runtime(
        vec![hedone],
        vec![
            hand_card(10, 31560181, false),
            hand_card(10, 31560111, false),
        ],
    );

    let error = runtime
        .try_plan_auto_round(&AutoRoundRequest::default())
        .unwrap_err();

    assert!(error.contains("invalid conduit auto-battle state"));

    let missing = team_runtime(
        vec![team_hero(10, 3156, 1_000, [31560111, 31560121])],
        vec![hand_card(10, 31560181, false)],
    );
    assert!(
        missing
            .try_plan_auto_round(&AutoRoundRequest::default())
            .unwrap_err()
            .contains("invalid conduit auto-battle state")
    );
}

#[test]
fn auto_round_plans_a_full_conduit_roster() {
    let mut runtime = team_runtime(
        vec![
            team_hero(10, 3145, 1_000, [31450111, 31450121]),
            team_hero(11, 3144, 1_000, [31440111, 31440121]),
            team_hero(12, 3149, 1_000, [31490111, 31490121]),
            team_hero(13, 3156, 1_000, [31560111, 31560121]),
        ],
        vec![
            hand_card(10, 31450181, false),
            hand_card(11, 31446012, false),
            hand_card(12, 31446011, false),
            hand_card(13, 31446012, false),
            hand_card(10, 31446022, false),
            hand_card(11, 31446011, false),
            hand_card(12, 31446012, false),
            hand_card(13, 31446011, false),
            hand_card(10, 31446021, false),
            hand_card(12, 31446022, false),
        ],
    );
    runtime.round_state.act_point = 5;

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(
        reply
            .opers
            .iter()
            .map(|oper| (
                oper.oper_type,
                oper.param1,
                oper.param2,
                oper.to_id,
                oper.param3,
                oper.card_param1.clone(),
            ))
            .collect::<Vec<_>>(),
        [2, 3, 5, 1, 1].map(|index| (Some(2), Some(index), None, Some(-1), None, None))
    );
    assert_eq!(
        reply
            .devices_opers
            .iter()
            .map(|oper| (oper.uid, oper.index))
            .collect::<Vec<_>>(),
        [
            (Some(10), Some(1)),
            (Some(11), Some(2)),
            (Some(12), Some(1)),
            (Some(13), Some(1))
        ]
    );
}
