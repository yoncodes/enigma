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
        let mut runtime = team_runtime(
            vec![beryl.clone(), marsha.clone(), lorentz.clone()],
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
    }
}
