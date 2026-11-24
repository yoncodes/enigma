use super::*;

#[test]
fn auto_round_uses_legal_low_hp_targets_without_mutating_the_hand() {
    let mut runtime = auto_runtime();
    let original = runtime.card_hand().to_vec();

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers.len(), 2);
    assert!(reply.opers.iter().all(|oper| {
        oper.oper_type == Some(CardOpType::PlayCard.id()) && oper.to_id == Some(-2)
    }));
    assert_eq!(runtime.card_hand(), original);
    runtime
        .build_begin_round_from_schedule(&BeginRoundRequest {
            opers: reply.opers,
            auto_oper: Some(true),
            ..Default::default()
        })
        .unwrap();
}

#[test]
fn auto_round_prefers_the_configured_win_target_over_lower_hp_enemies() {
    let mut win_target = enemy(-1, 100_000);
    win_target.model_id = Some(103370111);
    let mut add = enemy(-2, 100);
    add.model_id = Some(103370112);
    let mut runtime = team_runtime_against(
        vec![team_hero(10, 3066, 1_000, [30660111, 30660121])],
        vec![win_target, add],
        vec![hand_card(10, 30660111, false)],
    );
    runtime.fight.battle_id = Some(103370111);

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers[0].to_id, Some(-1));
}

#[test]
fn auto_round_keeps_the_selected_enemy_for_all_queued_attacks() {
    let mut runtime = team_runtime_against(
        vec![team_hero(10, 3066, 1_000, [30660111, 30660121])],
        vec![enemy(-1, 1), enemy(-2, 100_000)],
        vec![
            hand_card(10, 30660111, false),
            hand_card(10, 30660121, false),
        ],
    );
    runtime.round_state.act_point = 2;

    let reply = runtime.plan_auto_round(&AutoRoundRequest {
        to_id: Some(-1),
        ..Default::default()
    });

    assert_eq!(
        reply
            .opers
            .iter()
            .map(|operation| operation.to_id)
            .collect::<Vec<_>>(),
        vec![Some(-1), Some(-1)]
    );
}

#[test]
fn auto_round_keeps_support_targets_on_the_casters_team() {
    let mut runtime = auto_runtime();
    runtime.round_state.act_point = 2;

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert!(reply.opers.iter().any(|oper| oper.to_id == Some(10)));
    assert!(
        reply
            .opers
            .iter()
            .all(|oper| matches!(oper.to_id, Some(10 | -2)))
    );
}

#[test]
fn auto_round_attacks_the_enemy_without_the_cards_debuff() {
    let mut runtime = team_runtime_against(
        vec![team_hero(10, 3071, 1_000, [30710111, 30710121])],
        vec![enemy(-1, 100), enemy(-2, 1_000)],
        vec![hand_card(10, 30710111, false)],
    );
    grant_buff(&mut runtime, 10, -1, 30710111);

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers[0].to_id, Some(-2));
}
