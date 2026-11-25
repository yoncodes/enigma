use super::*;

#[test]
fn dynamo_opening_picks_channels_and_team_buffs_in_hand_order() {
    let attack = hand_card(10, 31430111, false);
    let group_buff = hand_card(11, 31350121, false);
    let scanning = hand_card(12, 31070111, false);
    let brown_noise = hand_card(12, 31070121, false);
    for (second, third) in [
        (scanning.clone(), group_buff.clone()),
        (group_buff, scanning),
    ] {
        let mut runtime = dynamo_runtime(vec![attack.clone(), second, third, brown_noise.clone()]);
        runtime.round_state.act_point = 3;
        let reply = runtime.plan_auto_round(&AutoRoundRequest::default());
        assert_eq!(reply.opers.len(), 3);
        assert!(reply.opers.iter().all(|op| op.param1 == Some(2)));
        assert!(reply.opers.iter().all(|op| op.to_id == Some(-1)));
        for count in [1, 2] {
            let continuation = runtime.plan_auto_round(&AutoRoundRequest {
                opers: reply.opers[..count].to_vec(),
                ..Default::default()
            });
            assert_eq!(continuation.opers.len(), 3 - count);
        }
        let mut invalid = reply.opers[0].clone();
        invalid.to_id = Some(-99);
        let rejected = runtime.plan_auto_round(&AutoRoundRequest {
            opers: vec![invalid],
            ..Default::default()
        });
        assert!(rejected.opers.is_empty());
    }
}

#[test]
fn dynamo_opening_keeps_normal_priority_outside_the_setup_comparison() {
    for (asleep, attack_skill, energy) in [
        (true, 31430111, 0),
        (false, 31430112, 0),
        (false, 31430111, 1),
    ] {
        let mut attack = hand_card(10, attack_skill, false);
        attack.energy = Some(energy);
        let mut runtime = dynamo_runtime(vec![
            attack,
            hand_card(11, 31350121, false),
            hand_card(12, 31070111, false),
            hand_card(11, 31350111, false),
        ]);
        if asleep {
            grant_buff(&mut runtime, -1, 12, 4031);
        }
        runtime.round_state.act_point = 1;
        let request = AutoRoundRequest::default();
        let opening = runtime.plan_auto_round(&request);
        runtime.fight.cur_round = Some(2);
        runtime.round_state.cur_round = 2;
        let regular = runtime.plan_auto_round(&request);
        assert_eq!(
            opening.opers, regular.opers,
            "asleep={asleep}, energy={energy}"
        );
        assert_eq!(opening.opers[0].param1, Some(1));
    }
}

fn dynamo_runtime(hand: Vec<CardInfo>) -> BattleRuntime {
    let mut enigma = team_hero(10, 3143, 1_000, [31430111, 31430124]);
    enigma.skill_group1.push(31430112);
    let mut runtime = team_runtime(
        vec![
            enigma,
            team_hero(11, 3135, 1_000, [31350111, 31350121]),
            team_hero(12, 3107, 1_000, [31070111, 31070121]),
        ],
        hand,
    );
    runtime.fight.cur_round = Some(1);
    runtime
}
