use super::*;

#[test]
fn auto_round_plays_energy_cards_before_other_cards() {
    let runtime = team_runtime(
        vec![
            team_hero(10, 3066, 1_000, [30660111, 30660121]),
            team_hero(11, 3144, 1_000, [31446011, 31446012]),
        ],
        vec![
            hand_card(10, 30660111, false),
            hand_card(11, 31446011, false),
        ],
    );

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    let played: Vec<_> = reply.opers.iter().map(|oper| oper.param1).collect();
    assert_eq!(played, vec![Some(2)]);
}

#[test]
fn auto_round_keeps_the_current_conduit_group_when_power_is_insufficient() {
    let insufficient = regulus_conduit_runtime(1_000).plan_auto_round(&AutoRoundRequest::default());
    assert_eq!(insufficient.devices_opers[0].index, Some(1));
}

#[test]
fn auto_round_chooses_current_utility_when_multiple_energy_types_are_available() {
    let mut runtime = twins_conduit_runtime(1_000, 0, false, 70);
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: [31446012, 31446021, 31446011, 31446011, 31446022, 31446022]
                .into_iter()
                .map(|skill_id| hand_card(10, skill_id, false))
                .collect(),
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    runtime.round_state.act_point = 2;

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers.len(), 2);
    assert_eq!(reply.devices_opers[0].index, Some(1));
}

#[test]
fn auto_round_prefers_flexible_energy_when_equal_amounts_are_usable() {
    let mut runtime = twins_conduit_runtime(1_000, 0, false, 70);
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: [31446022, 31446062]
                .into_iter()
                .map(|skill_id| hand_card(10, skill_id, false))
                .collect(),
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    runtime.round_state.act_point = 1;

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers[0].param1, Some(2));
}

#[test]
fn auto_round_prefers_temporary_special_energy_for_a_ready_conduit() {
    let mut runtime = twins_conduit_runtime(1_000, 1, false, 100);
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![
                hand_card(10, 31490211, true),
                hand_card(10, 31446022, false),
            ],
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    runtime.round_state.act_point = 1;

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers.len(), 1);
    assert_eq!(reply.opers[0].param1, Some(1));
}

#[test]
fn auto_round_skips_energy_without_a_selected_power_type() {
    let mut ally = team_hero(11, 3023, 1_000, [30230111, 30230121]);
    ally.ex_point = Some(5);
    ally.ex_point_max = Some(5);
    ally.ex_skill = Some(30230131);
    let runtime = coppelia_conduit_runtime(
        ally,
        vec![
            hand_card(11, 30230131, false),
            hand_card(11, 31446021, false),
            hand_card(10, 31446021, false),
        ],
        2,
    );

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(
        reply
            .opers
            .iter()
            .map(|oper| oper.param1)
            .collect::<Vec<_>>(),
        vec![Some(1)]
    );
}

#[test]
fn auto_round_selects_an_enemy_while_conduit_energy_resolves_on_its_owner() {
    let setup = |runtime: &mut BattleRuntime| {
        runtime
            .managers
            .execute_card(CardCommand::Setup(CardSetup {
                hand: vec![hand_card(10, 31446012, false)],
                draw_pile: Vec::new(),
                deck_num: 0,
            }))
            .unwrap();
        runtime.round_state.act_point = 1;
    };
    let mut runtime = twins_conduit_runtime_with_enemy_hps(&[1_000, 1_000], 0, false, 70);
    setup(&mut runtime);

    let reply = runtime.plan_auto_round(&AutoRoundRequest {
        to_id: Some(-2),
        ..Default::default()
    });

    assert_eq!(reply.opers[0].to_id, Some(-2));
    let pool = crate::engine::skill::target::TargetPool::from_fight_with_catalog(
        runtime.catalog_data.unwrap(),
        &runtime.fight,
    );
    let mut catalog = runtime.catalog.clone();
    runtime
        .managers
        .catalog()
        .extend_skill_roots(&mut catalog, [31446012], std::iter::empty());
    let mut determinism = runtime.determinism.clone();
    crate::engine::runtime::schedule::run_player_commands(
        &mut runtime.managers,
        &pool,
        &catalog,
        &mut determinism,
        crate::engine::skill::target::TargetContext::default(),
        reply
            .opers
            .iter()
            .filter_map(crate::engine::round::command::RoundCommand::from_oper),
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();
    assert_eq!(runtime.managers.conduit.power(1, 1), 2);

    let mut fallback = twins_conduit_runtime_with_enemy_hps(&[1_000, 0], 0, false, 70);
    setup(&mut fallback);

    let reply = fallback.plan_auto_round(&AutoRoundRequest {
        to_id: Some(-2),
        ..Default::default()
    });

    assert_eq!(reply.opers[0].to_id, Some(-1));
}
