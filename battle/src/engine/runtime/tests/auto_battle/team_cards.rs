use super::*;

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
