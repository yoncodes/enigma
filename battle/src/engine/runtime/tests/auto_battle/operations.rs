use super::*;

#[test]
fn auto_round_honors_existing_operations_without_echoing_them() {
    let mut runtime = auto_runtime();
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

    assert_eq!(reply.opers.len(), 1);
    assert_eq!(reply.opers[0].param1, Some(2));
    let mut opers = request.opers;
    opers.extend(reply.opers);
    runtime
        .advance_round(BeginRoundRequest {
            opers,
            auto_oper: Some(true),
            ..Default::default()
        })
        .unwrap();
}

#[test]
fn auto_round_ignores_unused_skill_choice_on_move() {
    let mut runtime = auto_runtime();
    runtime.round_state.act_point = 2;
    let planner = runtime.auto_round_planner();
    let mut request = AutoRoundRequest {
        opers: vec![BeginRoundOper {
            oper_type: Some(CardOpType::MoveCard.id()),
            param1: Some(1),
            param2: Some(2),
            ..Default::default()
        }],
        ..Default::default()
    };
    let expected = planner.plan(&request).unwrap();
    request.opers[0].param3 = Some(i32::MAX);

    assert!(!expected.opers.is_empty());
    assert_eq!(planner.plan(&request).unwrap(), expected);
}

#[test]
fn auto_round_distinguishes_an_invalid_choice_from_missing_skill_data() {
    let mut runtime = auto_runtime();
    let request = AutoRoundRequest {
        opers: vec![BeginRoundOper {
            oper_type: Some(CardOpType::PlayCard.id()),
            param1: Some(1),
            param3: Some(i32::MAX),
            to_id: Some(-1),
            ..Default::default()
        }],
        ..Default::default()
    };
    assert!(matches!(
        runtime.auto_round_planner().plan(&request),
        Err(AutoRoundError::InvalidRequest)
    ));

    runtime.catalog = SkillEffectCatalog::default();
    assert!(matches!(
        runtime.auto_round_planner().plan(&AutoRoundRequest::default()),
        Err(AutoRoundError::Engine(message)) if message.contains("MissingSkill(30230111)")
    ));
}

#[test]
fn both_round_routes_reject_a_loaded_choice_from_another_card_rank() {
    let mut hero = team_hero(10, 3120, 1_000, [31200111, 312001215]);
    hero.skill_group2.push(312001225);
    let mut runtime = team_runtime(vec![hero], vec![hand_card(10, 312001215, false)]);
    assert!(runtime.catalog.get(31200165).is_some());

    let planner = runtime.auto_round_planner();
    let mut request = AutoRoundRequest {
        opers: vec![BeginRoundOper {
            oper_type: Some(CardOpType::PlayCard.id()),
            param1: Some(1),
            param3: Some(31200164),
            to_id: Some(-1),
            ..Default::default()
        }],
        ..Default::default()
    };
    assert!(planner.plan(&request).is_ok());
    runtime
        .clone()
        .advance_round(BeginRoundRequest {
            opers: request.opers.clone(),
            ..Default::default()
        })
        .unwrap();

    request.opers[0].param3 = Some(31200165);
    assert!(matches!(
        planner.plan(&request),
        Err(AutoRoundError::InvalidRequest)
    ));
    let prior_hand = runtime.managers.card.hand().to_vec();
    let error = runtime
        .advance_round(BeginRoundRequest {
            opers: request.opers,
            ..Default::default()
        })
        .unwrap_err();
    assert!(error.contains("InvalidPlaySkill"), "{error}");
    assert_eq!(runtime.managers.card.hand(), prior_hand);
}

#[test]
fn auto_round_indices_replay_through_the_real_round_after_merges() {
    let mut runtime = team_runtime(
        vec![team_hero(10, 3023, 1_000, [30230111, 30230121])],
        vec![
            hand_card(10, 30230111, false),
            hand_card(10, 30230121, false),
            hand_card(10, 30230111, false),
            hand_card(10, 30230121, false),
        ],
    );
    runtime.round_state.act_point = 3;

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    let played: Vec<_> = reply
        .opers
        .iter()
        .map(|oper| (oper.param1, oper.to_id))
        .collect();
    assert_eq!(
        played,
        vec![
            (Some(1), Some(-1)),
            (Some(2), Some(-1)),
            (Some(1), Some(10))
        ]
    );
    runtime
        .advance_round(BeginRoundRequest {
            opers: reply.opers,
            auto_oper: Some(true),
            ..Default::default()
        })
        .unwrap();
}

#[test]
fn auto_round_picks_one_of_a_choice_cards_options() {
    let runtime_with_seed = |seed| {
        let mut runtime = team_runtime(
            vec![team_hero(10, 3120, 1_000, [31200111, 312001215])],
            vec![hand_card(10, 312001215, false)],
        );
        runtime.determinism = RoundDeterminism::with_seed(seed);
        runtime
    };
    let picks: std::collections::HashSet<_> = (1..=20)
        .map(|seed| {
            runtime_with_seed(seed)
                .plan_auto_round(&AutoRoundRequest::default())
                .opers[0]
                .param3
        })
        .collect();
    assert_eq!(picks, [Some(31200164), Some(31200231)].into());

    let mut runtime = runtime_with_seed(1);
    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());
    assert_eq!(reply.opers.len(), 1);
    runtime
        .auto_round_planner()
        .plan(&AutoRoundRequest {
            opers: reply.opers.clone(),
            ..Default::default()
        })
        .unwrap();
    runtime
        .advance_round(BeginRoundRequest {
            opers: reply.opers,
            auto_oper: Some(true),
            ..Default::default()
        })
        .unwrap();
}

#[test]
fn auto_round_keeps_the_captured_empty_cloth_skill_envelope() {
    let runtime = team_runtime(
        vec![team_hero(10, 3149, 1_000, [31490111, 31490121])],
        vec![hand_card(10, 31446011, false)],
    );

    assert_eq!(
        runtime
            .plan_auto_round(&AutoRoundRequest::default())
            .cloth_skill,
        Some(sonettobuf::ClothSkillInfo {
            skill_id: Some(0),
            from_id: Some(0),
            to_id: Some(0),
            r#type: Some(0),
        })
    );
}
