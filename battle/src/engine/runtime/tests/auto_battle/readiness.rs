use super::*;

#[test]
fn auto_round_skips_an_ultimate_without_its_required_resource() {
    let mut runtime = auto_runtime();
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![
                CardInfo {
                    uid: Some(10),
                    skill_id: Some(30230131),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(10),
                    skill_id: Some(30230111),
                    ..Default::default()
                },
            ],
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers.len(), 1);
    assert_eq!(reply.opers[0].param1, Some(2));
    runtime
        .advance_round(BeginRoundRequest {
            opers: reply.opers,
            auto_oper: Some(true),
            ..Default::default()
        })
        .unwrap();
}

#[test]
fn auto_round_casts_a_ready_ultimate_before_other_cards() {
    crate::test_support::init_config();
    let fight = Fight {
        battle_id: Some(77),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3066),
                team_type: Some(1),
                position: Some(1),
                current_hp: Some(1_000),
                ex_point: Some(0),
                ex_skill: Some(30660131),
                skill_group1: vec![30660111],
                skill_group2: vec![30660121],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                position: Some(1),
                current_hp: Some(1_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = runtime(fight);
    runtime.round_state.act_point = 1;
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: [30660111, 30660121, 30660131]
                .map(|skill_id| CardInfo {
                    uid: Some(10),
                    skill_id: Some(skill_id),
                    ..Default::default()
                })
                .to_vec(),
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    runtime.managers.ex_point.set(10, 10, 5, 0);

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers.len(), 1);
    assert_eq!(reply.opers[0].param1, Some(3));
    runtime
        .advance_round(BeginRoundRequest {
            opers: reply.opers,
            auto_oper: Some(true),
            ..Default::default()
        })
        .unwrap();
}

#[test]
fn auto_round_prefers_an_owner_bound_temporary_skill() {
    let mut runtime = team_runtime(
        vec![
            team_hero(10, 3124, 1_000, [312431212, 312451115]),
            team_hero(11, 3123, 1_000, [312301142, 312301242]),
        ],
        vec![
            CardInfo {
                uid: Some(10),
                skill_id: Some(312451011),
                temp_card: Some(true),
                card_type: Some(sonettobuf::card_info::CardType::Skill3 as i32),
                hero_id: Some(3124),
                ..Default::default()
            },
            hand_card(11, 312301142, false),
        ],
    );
    runtime.round_state.act_point = 1;

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers[0].param1, Some(1));
}

#[test]
fn auto_round_plays_hero_precasts_first_and_leaves_stage_cards_alone() {
    let runtime = support_runtime(
        1_000,
        vec![
            hand_card(10, 30230111, false),
            hand_card(10, 30820111, true),
            hand_card(10, 30230121, true),
        ],
    );

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    let played: Vec<_> = reply.opers.iter().map(|oper| oper.param1).collect();
    assert_eq!(played, vec![Some(3), Some(1)]);
}

#[test]
fn auto_round_casts_ready_ultimates_left_to_right() {
    let mut thirty_seven = team_hero(10, 3066, 1_000, [30660111, 30660121]);
    thirty_seven.ex_skill = Some(30660131);
    let mut tooth_fairy = team_hero(11, 3053, 1_000, [30530111, 30530121]);
    tooth_fairy.ex_skill = Some(30530131);
    let mut runtime = team_runtime(
        vec![thirty_seven, tooth_fairy],
        vec![
            hand_card(10, 30660131, false),
            hand_card(11, 30530131, false),
        ],
    );
    runtime.managers.ex_point.set(10, 10, 5, 0);
    runtime.managers.ex_point.set(11, 11, 5, 0);

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers[0].param1, Some(1));
}

#[test]
fn auto_round_plays_cards_made_free_by_bendith_first() {
    let mut runtime = team_runtime(
        vec![
            team_hero(10, 3146, 1_000, [31460111, 31460121]),
            team_hero(11, 3066, 1_000, [30660111, 30660121]),
        ],
        vec![
            hand_card(11, 30660111, false),
            hand_card(10, 31460121, false),
        ],
    );
    grant_buff(&mut runtime, 10, 10, 31460133);

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers[0].param1, Some(2));
}

#[test]
fn auto_round_spends_no_action_points_for_all_bendith_owner_skills() {
    crate::test_support::init_config();
    let fight = Fight {
        battle_id: Some(77),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3146),
                    team_type: Some(1),
                    position: Some(1),
                    current_hp: Some(1_000),
                    ex_point: Some(5),
                    ex_skill: Some(31460131),
                    skill_group1: vec![31460111],
                    skill_group2: vec![31460121],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    model_id: Some(3023),
                    team_type: Some(1),
                    position: Some(2),
                    current_hp: Some(1_000),
                    ex_point: Some(0),
                    ex_skill: Some(30230131),
                    skill_group1: vec![30230111],
                    skill_group2: vec![30230121],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                position: Some(1),
                current_hp: Some(1_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = runtime(fight);
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![
                CardInfo {
                    uid: Some(10),
                    skill_id: Some(31460131),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(10),
                    skill_id: Some(31460111),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(11),
                    skill_id: Some(30230111),
                    ..Default::default()
                },
            ],
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    runtime.round_state.act_point = 1;
    runtime
        .managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60001, "AddBuff"),
            },
            source_uid: 10,
            target_uid: 10,
            buff_id: 31460133,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers.len(), 3);
    let mut hand = runtime.card_hand().to_vec();
    let chosen = reply
        .opers
        .iter()
        .map(|oper| {
            let index = oper.param1.unwrap() as usize - 1;
            hand.remove(index).skill_id.unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(chosen, vec![31460131, 31460111, 30230111]);
}
