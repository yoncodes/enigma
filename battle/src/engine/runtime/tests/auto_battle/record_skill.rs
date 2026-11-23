use super::*;

#[test]
fn auto_round_records_the_casters_attack_before_their_higher_rank_buff() {
    check_recording(true, 31390114);
}

#[test]
fn auto_round_records_the_first_eligible_own_buff_without_own_attack() {
    check_recording(false, 31390122);
}

fn check_recording(own_attack: bool, recorded_skill: i32) {
    let mut lorentz = team_hero(10, 3139, 1_000, [31390114, 31390121]);
    lorentz.ex_skill = Some(31390132);
    let beryl = team_hero(11, 3134, 1_000, [31340111, 31340121]);
    let mut hand = vec![
        hand_card(11, 31340112, false),
        hand_card(10, 31390122, false),
        hand_card(10, 31390121, false),
        hand_card(10, 31390132, false),
    ];
    if own_attack {
        hand.push(hand_card(10, 31390114, false));
    }
    let mut runtime = team_runtime_against(
        vec![lorentz, beryl],
        vec![enemy(-1, 1_000), enemy(-2, 1_000)],
        hand,
    );
    grant_buff(&mut runtime, 10, 10, 235002);
    runtime.managers.ex_point.set(10, 10, 5, 0);
    runtime.round_state.act_point = 1;

    let default = runtime.plan_auto_round(&AutoRoundRequest::default());
    assert_eq!(default.opers[0].to_id, Some(-1));
    let reply = runtime.plan_auto_round(&AutoRoundRequest {
        to_id: Some(-2),
        ..Default::default()
    });

    let [operation] = reply.opers.as_slice() else {
        panic!("expected one selected card");
    };
    assert_eq!(operation.param1, Some(4));
    assert_eq!(operation.to_id, Some(-2));
    assert_eq!(operation.card_param1, Some(format!("{recorded_skill}#10")));
    let round = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest {
            opers: reply.opers,
            auto_oper: Some(true),
            ..Default::default()
        })
        .unwrap();
    let ultimate = round
        .fight_step
        .iter()
        .find(|step| step.act_id == Some(31390132) && step.card_index == Some(1))
        .unwrap();
    assert_eq!(ultimate.to_id, Some(-2));
    let record = format!("3,{recorded_skill},10");
    let recording = ultimate
        .act_effect
        .iter()
        .position(|effect| {
            effect.target_id == Some(10)
                && effect.buff_act_info.as_ref().is_some_and(|info| {
                    info.act_id == Some(1104) && info.str_param.as_deref() == Some(record.as_str())
                })
        })
        .expect("recorded incantation");
    let previews = ultimate
        .act_effect
        .iter()
        .enumerate()
        .filter(|(_, effect)| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Clienteffect as i32)
        })
        .collect::<Vec<_>>();
    let [(index, preview)] = previews.as_slice() else {
        panic!("expected one copy preview");
    };
    assert!(recording < *index);
    assert_eq!(preview.target_id, Some(10));
    assert_eq!(preview.effect_num, Some(5));
    assert_eq!(preview.config_effect, Some(0));
}
