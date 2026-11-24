use super::*;

#[test]
fn auto_round_heals_an_ally_below_eighty_percent_before_attacking() {
    let hand = || {
        vec![
            hand_card(11, 30820111, false),
            hand_card(11, 30820121, false),
        ]
    };

    let hurt = support_runtime(500, hand());
    let reply = hurt.plan_auto_round(&AutoRoundRequest::default());
    assert_eq!(reply.opers.len(), 1);
    assert_eq!(
        (reply.opers[0].param1, reply.opers[0].to_id),
        (Some(2), Some(11))
    );

    let healthy = support_runtime(1_000, hand());
    let reply = healthy.plan_auto_round(&AutoRoundRequest::default());
    assert_eq!(reply.opers[0].param1, Some(1));
}

#[test]
fn auto_round_sees_a_planned_heal_before_choosing_the_next_card() {
    let mut healer = team_hero(11, 3082, 1_000, [30820111, 30820121]);
    healer.attr = Some(HeroAttribute {
        hp: Some(1_000),
        attack: Some(1_000),
        ..Default::default()
    });
    let mut runtime = team_runtime(
        vec![team_hero(10, 3023, 790, [30230111, 30230121]), healer],
        vec![
            hand_card(11, 30820121, false),
            hand_card(10, 30230111, false),
            hand_card(11, 30820121, false),
        ],
    );
    runtime.round_state.act_point = 2;

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    let played: Vec<_> = reply
        .opers
        .iter()
        .map(|oper| (oper.param1, oper.to_id))
        .collect();
    assert_eq!(played, vec![(Some(1), Some(10)), (Some(1), Some(-1))]);
}

#[test]
fn auto_round_prefers_an_explicit_counter_card_over_an_attack() {
    let runtime = team_runtime(
        vec![team_hero(10, 3123, 1_000, [312301142, 312301242])],
        vec![
            hand_card(10, 312301142, false),
            hand_card(10, 312301242, false),
        ],
    );

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers[0].param1, Some(2));
}

#[test]
fn auto_round_prefers_hp_redistribution_setup_over_an_attack() {
    let mut fatutu = team_hero(10, 3109, 1_000, [31090111, 31090121]);
    fatutu.buffs.push(sonettobuf::BuffInfo {
        uid: Some(20),
        buff_id: Some(31090111),
        layer: Some(8),
        ..Default::default()
    });
    let runtime = team_runtime(
        vec![fatutu, team_hero(11, 3124, 1_000, [312431212, 312451115])],
        vec![
            hand_card(10, 31090111, false),
            hand_card(10, 31090121, false),
        ],
    );

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers[0].param1, Some(2));
}

#[test]
fn auto_round_gives_every_hero_a_card_before_repeating_one() {
    let mut runtime = team_runtime(
        vec![
            team_hero(10, 3066, 1_000, [30660111, 30660121]),
            team_hero(11, 3074, 1_000, [30740111, 30740121]),
        ],
        vec![
            hand_card(10, 30660111, false),
            hand_card(10, 30660121, false),
            hand_card(11, 30740111, false),
        ],
    );
    runtime.round_state.act_point = 2;

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    let played: Vec<_> = reply.opers.iter().map(|oper| oper.param1).collect();
    assert_eq!(played, vec![Some(1), Some(2)]);
}

#[test]
fn auto_round_puts_an_urgent_heal_before_other_heroes_first_cards() {
    for (ally_hp, expected) in [(500, (Some(1), Some(10))), (700, (Some(2), Some(-1)))] {
        let mut runtime = team_runtime(
            vec![
                team_hero(10, 3066, ally_hp, [30660111, 30660121]),
                team_hero(11, 3082, 1_000, [30820111, 30820121]),
            ],
            vec![
                hand_card(11, 30820111, false),
                hand_card(11, 30820121, false),
                hand_card(10, 30660111, false),
            ],
        );
        runtime.round_state.act_point = 2;
        let request = AutoRoundRequest {
            opers: vec![BeginRoundOper {
                oper_type: Some(CardOpType::PlayCard.id()),
                param1: Some(1),
                to_id: Some(-1),
                ..Default::default()
            }],
            to_id: Some(-1),
        };

        let reply = runtime.plan_auto_round(&request);

        assert_eq!(reply.opers.len(), 1, "ally hp {ally_hp}");
        assert_eq!(
            (reply.opers[0].param1, reply.opers[0].to_id),
            expected,
            "ally hp {ally_hp}"
        );
    }
}

#[test]
fn auto_round_skips_buffs_every_target_already_holds() {
    let team = || {
        vec![
            team_hero(10, 3091, 1_000, [30910111, 30910121]),
            team_hero(11, 3072, 500, [30720111, 30720121]),
        ]
    };
    let grant = |runtime: &mut BattleRuntime, target_uid, buff_ids: &[i32]| {
        for &buff_id in buff_ids {
            grant_buff(runtime, 10, target_uid, buff_id);
        }
    };

    let mut runtime = team_runtime(
        vec![
            team_hero(10, 3106, 1_000, [31060111, 31060121]),
            team_hero(11, 3072, 500, [30720111, 30720121]),
        ],
        vec![hand_card(10, 31060121, false)],
    );
    grant(&mut runtime, 11, &[31060002]);
    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());
    assert_eq!(reply.opers[0].to_id, Some(10));

    let lorelei = [30910111, 30910121];
    let mut runtime = team_runtime(
        team(),
        vec![
            hand_card(10, 30910121, false),
            hand_card(11, 30720121, false),
        ],
    );
    grant(&mut runtime, 10, &lorelei);
    grant(&mut runtime, 11, &lorelei);
    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());
    assert_eq!(reply.opers[0].param1, Some(2));
}

#[test]
fn auto_round_never_puts_debuff_cards_last() {
    let mut runtime = team_runtime(
        vec![
            team_hero(10, 3098, 1_000, [30980111, 30980121]),
            team_hero(11, 3091, 1_000, [30910111, 30910121]),
        ],
        vec![
            hand_card(10, 30980121, false),
            hand_card(11, 30910121, false),
        ],
    );
    grant_buff(&mut runtime, 10, -1, 30980121);

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers[0].param1, Some(1));
}

#[test]
fn auto_round_ignores_conditional_buffs_when_judging_redundancy() {
    let mut runtime = team_runtime(
        vec![
            team_hero(10, 3087, 1_000, [30870111, 30870121]),
            team_hero(11, 3091, 1_000, [30910111, 30910121]),
        ],
        vec![
            hand_card(10, 30870111, false),
            hand_card(11, 30910121, false),
        ],
    );
    for target_uid in [10, 11] {
        for buff_id in [30870111, 30870121] {
            grant_buff(&mut runtime, 10, target_uid, buff_id);
        }
    }

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers[0].param1, Some(1));
}
