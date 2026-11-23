use super::*;

#[test]
fn auto_round_orders_sovereign_charge_between_ultimates() {
    let mut beryl = team_hero(10, 3134, 1_000, [31340111, 31340121]);
    beryl.ex_point = Some(5);
    beryl.ex_skill = Some(31340131);
    let mut marsha = team_hero(11, 3127, 1_000, [31270111, 31270121]);
    marsha.ex_point = Some(5);
    marsha.ex_skill = Some(31270132);
    let mut lorentz = team_hero(12, 3139, 1_000, [31390114, 31390122]);
    lorentz.ex_point = Some(5);
    lorentz.ex_skill = Some(31390132);

    for charge in [31340161, 31340162, 31340163] {
        let mut sovereign = hand_card(10, charge, true);
        sovereign.card_type = Some(sonettobuf::card_info::CardType::Skill3 as i32);
        let mut runtime = team_runtime_against(
            vec![beryl.clone(), marsha.clone(), lorentz.clone()],
            vec![enemy(-1, 1_000), enemy(-2, 1_000)],
            vec![
                sovereign,
                hand_card(10, 31340131, false),
                hand_card(11, 31270132, false),
                hand_card(12, 31390132, false),
            ],
        );
        runtime.round_state.act_point = 3;
        let plan = runtime
            .auto_round_planner()
            .plan(&AutoRoundRequest::default())
            .unwrap();
        let picks: Vec<_> = plan
            .opers
            .iter()
            .map(|operation| operation.param1)
            .collect();
        assert_eq!(picks, [Some(3), Some(3), Some(1), Some(1)]);
        assert_eq!(plan.opers[2].to_id, Some(-1));

        let aimed = runtime
            .auto_round_planner()
            .plan(&AutoRoundRequest {
                to_id: Some(-2),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(aimed.opers[2].to_id, Some(-2));

        let mut charge = aimed.opers[2].clone();
        charge.param1 = Some(1);
        let continued = runtime
            .auto_round_planner()
            .plan(&AutoRoundRequest {
                opers: vec![charge],
                to_id: Some(-2),
            })
            .unwrap();
        assert_eq!(continued.opers.len(), 3);
    }
}

#[test]
fn auto_round_plays_configured_vision_after_one_opening_ultimate() {
    let mut beryl = team_hero(10, 3134, 1_000, [31340111, 31340121]);
    let mut marsha = team_hero(11, 3127, 1_000, [31270111, 31270121]);
    marsha.ex_point = Some(5);
    marsha.ex_skill = Some(31270132);
    let mut lorentz = team_hero(12, 3139, 1_000, [31390114, 31390122]);
    lorentz.ex_point = Some(5);
    lorentz.ex_skill = Some(31390132);
    let mut vision = hand_card(10, 31340165, true);
    vision.card_type = Some(sonettobuf::card_info::CardType::Skill3 as i32);

    for (configured, has_ultimates, extra_free_card, expected) in [
        (true, true, false, vec![2, 1, 1]),
        (true, true, true, vec![4, 2, 1, 1]),
        (true, false, false, vec![1, 1]),
        (false, true, false, vec![1, 1, 1]),
    ] {
        beryl.passive_skill = if configured { vec![31340141] } else { vec![] };
        let mut hand = if has_ultimates {
            vec![
                vision.clone(),
                hand_card(11, 31270132, false),
                hand_card(12, 31390132, false),
            ]
        } else {
            vec![vision.clone(), hand_card(10, 31340111, false)]
        };
        if extra_free_card {
            hand.push(hand_card(12, 31390122, true));
        }
        let mut runtime = team_runtime_against(
            vec![beryl.clone(), marsha.clone(), lorentz.clone()],
            vec![enemy(-1, 1_000)],
            hand,
        );
        runtime.round_state.act_point = 2;
        let plan = runtime
            .auto_round_planner()
            .plan(&AutoRoundRequest::default())
            .unwrap();
        let picks: Vec<_> = plan.opers.iter().map(|op| op.param1.unwrap()).collect();
        assert_eq!(
            picks, expected,
            "configured={configured} ultimates={has_ultimates} extra_free_card={extra_free_card}"
        );
    }
}

#[test]
fn auto_round_aims_configured_vision_at_enemies() {
    let mut beryl = team_hero(10, 3134, 1_000, [31340111, 31340121]);
    for (skill, configured, aim) in [
        (31340164, true, Some(-1)),
        (31340165, true, Some(-1)),
        (31340166, true, Some(-1)),
        (31340165, false, Some(10)),
        (31340168, true, Some(10)),
    ] {
        beryl.passive_skill = if configured { vec![31340141] } else { vec![] };
        let mut crystal = hand_card(10, skill, true);
        crystal.card_type = Some(sonettobuf::card_info::CardType::Skill3 as i32);
        let mut runtime = team_runtime_against(
            vec![beryl.clone()],
            vec![enemy(-1, 1_000), enemy(-2, 1_000)],
            vec![crystal, hand_card(10, 31340111, false)],
        );
        runtime.round_state.act_point = 1;
        let plan = runtime
            .auto_round_planner()
            .plan(&AutoRoundRequest::default())
            .unwrap();
        assert_eq!(plan.opers[0].param1, Some(1));
        assert_eq!(plan.opers[0].to_id, aim);

        if aim == Some(-1) {
            let aimed = runtime
                .auto_round_planner()
                .plan(&AutoRoundRequest {
                    to_id: Some(-2),
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(aimed.opers[0].param1, Some(1));
            assert_eq!(aimed.opers[0].to_id, Some(-2));
            let continued = runtime
                .auto_round_planner()
                .plan(&AutoRoundRequest {
                    opers: vec![aimed.opers[0].clone()],
                    to_id: Some(-2),
                })
                .unwrap();
            assert_eq!(continued.opers.len(), 1);

            if skill == 31340165 {
                let round = runtime
                    .build_begin_round_from_schedule(&BeginRoundRequest {
                        opers: aimed.opers,
                        auto_oper: Some(true),
                        ..Default::default()
                    })
                    .unwrap();
                let vision = round
                    .fight_step
                    .iter()
                    .find(|step| step.act_id == Some(skill) && step.card_index == Some(1))
                    .unwrap();
                assert_eq!(vision.to_id, Some(-2));
                assert_eq!(runtime.managers.buff.buff_type_layer(10, 31340002), 3);
                assert_eq!(runtime.managers.buff.buff_type_layer(-2, 31340002), 0);
            }
        }
    }
}
