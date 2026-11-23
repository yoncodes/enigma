use super::*;

#[test]
fn auto_round_leads_with_the_second_incantation_in_the_opening() {
    let beryl = team_hero(10, 3134, 1_000, [31340111, 31340121]);
    let mut runtime = team_runtime(
        vec![beryl],
        vec![
            hand_card(10, 31340111, false),
            hand_card(10, 31340121, false),
        ],
    );
    runtime.fight.cur_round = Some(1);
    let opening = runtime.plan_auto_round(&AutoRoundRequest::default());
    assert_eq!(opening.opers[0].param1, Some(2));

    runtime.fight.cur_round = Some(2);
    runtime.round_state.cur_round = 2;
    let later = runtime.plan_auto_round(&AutoRoundRequest::default());
    assert_eq!(later.opers[0].param1, Some(1));
}

#[test]
fn auto_round_opens_defense_before_an_ordinary_attack() {
    let enemy_uids = [-1, -2];
    for (energy, already_weakened, expected_pick) in [(0, false, 2), (1, false, 1), (0, true, 1)] {
        let mut attack = hand_card(11, 31270111, false);
        attack.energy = Some(energy);
        let mut runtime = team_runtime_against(
            vec![
                team_hero(10, 3134, 1_000, [31340111, 31340121]),
                team_hero(11, 3127, 1_000, [31270111, 31270121]),
                team_hero(12, 3081, 1_000, [30810481, 30810226]),
            ],
            enemy_uids
                .into_iter()
                .map(|uid| enemy(uid, 1_000))
                .collect(),
            vec![
                hand_card(10, 31340121, false),
                attack,
                hand_card(12, 30810226, false),
            ],
        );
        if already_weakened {
            for uid in enemy_uids {
                grant_buff(&mut runtime, 12, uid, 30810105);
                change_buff_duration(&mut runtime, uid, 30810105, -1);
            }
        }
        runtime.fight.cur_round = Some(1);
        runtime.round_state.act_point = 2;
        let reply = runtime.plan_auto_round(&AutoRoundRequest::default());
        let played: Vec<_> = reply
            .opers
            .iter()
            .map(|operation| operation.param1)
            .collect();
        assert_eq!(
            played,
            vec![Some(1), Some(expected_pick)],
            "energy={energy}, already_weakened={already_weakened}"
        );
    }
}

#[test]
fn auto_round_aims_source_buffs_at_the_selected_enemy() {
    let beryl = team_hero(10, 3134, 1_000, [31340111, 31340121]);
    let mut runtime = team_runtime_against(
        vec![beryl],
        vec![enemy(-1, 1_000), enemy(-2, 1_000)],
        vec![hand_card(10, 31340121, false)],
    );
    let default = runtime.plan_auto_round(&AutoRoundRequest::default());
    assert_eq!(default.opers[0].to_id, Some(-1));

    let reply = runtime.plan_auto_round(&AutoRoundRequest {
        to_id: Some(-2),
        ..Default::default()
    });
    assert_eq!(reply.opers[0].to_id, Some(-2));
    let round = runtime
        .advance_round(BeginRoundRequest {
            opers: reply.opers,
            auto_oper: Some(true),
            ..Default::default()
        })
        .unwrap();
    let cast = round
        .fight_step
        .iter()
        .find(|step| step.act_id == Some(31340121))
        .unwrap();
    assert!(cast.act_effect.iter().any(|effect| {
        effect.target_id == Some(10)
            && effect
                .buff
                .as_ref()
                .is_some_and(|buff| buff.buff_id == Some(31340002))
    }));
}

#[test]
fn auto_round_source_buff_aim_respects_the_enemy_selection_gate() {
    let beryl = team_hero(10, 3134, 1_000, [31340111, 31340121]);
    let mut runtime = team_runtime_against(
        vec![beryl],
        vec![enemy(-1, 1_000), enemy(-2, 1_000), enemy(-3, 1_000)],
        vec![
            hand_card(10, 31340121, false),
            hand_card(10, 31340111, false),
        ],
    );
    runtime.fight.cur_round = Some(1);
    runtime.round_state.act_point = 2;
    grant_buff(&mut runtime, -1, -1, 24211);
    grant_buff(&mut runtime, -2, -2, 11410101);

    let planner = runtime.auto_round_planner();
    let mut request = AutoRoundRequest {
        to_id: Some(-2),
        ..Default::default()
    };
    let planned = planner.plan(&request).unwrap();
    assert_eq!(planned.opers[0].param1, Some(1));
    assert_eq!(planned.opers[0].to_id, Some(-3));

    request.opers = vec![BeginRoundOper {
        oper_type: Some(CardOpType::PlayCard.id()),
        param1: Some(1),
        to_id: Some(-3),
        ..Default::default()
    }];
    assert_eq!(planner.plan(&request).unwrap().opers.len(), 1);

    for target in [-1, -2] {
        request.opers[0].to_id = Some(target);
        assert!(planner.plan(&request).unwrap().opers.is_empty());
    }
}

#[test]
fn auto_round_plays_a_ready_team_ultimate_at_its_visible_index() {
    let mut lorentz = team_hero(10, 3139, 1_000, [31390111, 31390121]);
    lorentz.ex_point = Some(5);
    lorentz.ex_skill = Some(31390131);
    lorentz.buffs.push(sonettobuf::BuffInfo {
        uid: Some(20),
        buff_id: Some(31390181),
        from_uid: Some(10),
        ..Default::default()
    });
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![lorentz],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![enemy(-1, 1_000)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = runtime(fight);
    runtime.start_round().unwrap();
    let cards = &runtime.managers.card;
    assert_eq!(cards.team_cards()[0].skill_id, Some(31390131));
    let visible_index = cards.hand().len() as i32 + 1;

    let reply = runtime
        .auto_round_planner()
        .plan(&AutoRoundRequest::default())
        .unwrap();
    assert_eq!(reply.opers[0].param1, Some(visible_index));

    let round = runtime
        .advance_round(BeginRoundRequest {
            opers: reply.opers,
            auto_oper: Some(true),
            ..Default::default()
        })
        .unwrap();
    assert!(
        round
            .fight_step
            .iter()
            .any(|step| step.act_id == Some(31390131))
    );
}

#[test]
fn auto_round_holds_fractal_attacks_for_ready_ultimates() {
    use crate::engine::manager::card::EnchantedType;

    let mut beryl = team_hero(10, 3134, 1_000, [31340111, 31340121]);
    beryl.ex_point = Some(5);
    beryl.ex_skill = Some(31340131);
    let lorentz = team_hero(11, 3139, 1_000, [31390114, 31390122]);
    for (skill, free_ultimate, first) in [
        (31390114, false, 2),
        (31390122, false, 1),
        (31390114, true, 2),
    ] {
        let mut copy = hand_card(11, skill, true);
        copy.enchants.push(sonettobuf::CardEnchant {
            enchant_id: Some(EnchantedType::Lorenz.id()),
            duration: Some(-1),
            ..Default::default()
        });
        let mut runtime = team_runtime(
            vec![beryl.clone(), lorentz.clone()],
            vec![copy, hand_card(10, 31340131, false)],
        );
        runtime.round_state.act_point = i32::from(!free_ultimate);
        if free_ultimate {
            grant_buff(&mut runtime, 10, 10, 6270589);
        }

        let plan = runtime
            .auto_round_planner()
            .plan(&AutoRoundRequest::default())
            .unwrap();
        assert_eq!(plan.opers.len(), 2);
        assert_eq!(plan.opers[0].param1, Some(first));
    }
}
