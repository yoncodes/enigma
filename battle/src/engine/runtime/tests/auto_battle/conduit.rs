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
