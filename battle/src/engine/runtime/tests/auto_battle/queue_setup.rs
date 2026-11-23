use super::*;
use crate::engine::{
    event::kind::EventKind,
    skill::{condition::parse_conditions, rule::route::ConditionRoute},
};

#[test]
fn auto_round_prepares_an_unheld_buff_when_its_queue_rank_reaches_allies() {
    for (matching, passive, held, conditional, after_damage, expected) in [
        (true, true, false, false, true, 2),
        (true, false, false, false, true, 1),
        (true, true, true, false, true, 1),
        (false, true, false, false, true, 1),
        (true, true, false, true, true, 1),
        (true, true, false, false, false, 1),
    ] {
        let mut lorentz = team_hero(13, 3139, 1_000, [31390114, 31390121]);
        lorentz.skill_group2.push(31390122);
        if passive {
            lorentz.passive_skill.push(31390141);
        }
        let mut hand = vec![
            hand_card(12, 30810226, false),
            hand_card(11, 31270111, false),
            hand_card(13, 31390121, false),
        ];
        if matching {
            hand.insert(0, hand_card(10, 31340121, false));
        }
        let mut runtime = team_runtime(
            vec![
                team_hero(10, 3134, 1_000, [31340111, 31340121]),
                team_hero(11, 3127, 1_000, [31270111, 31270121]),
                team_hero(12, 3081, 1_000, [30810481, 30810226]),
                lorentz,
            ],
            hand,
        );
        if conditional {
            let mut effect = runtime.catalog.get(31390141).unwrap().clone();
            let slot = &mut effect.slots[1];
            slot.conditions = parse_conditions(
                runtime.managers.catalog().game_data(),
                "1061&19012#31390121|1061&19012#31390122",
            );
            slot.compiled_route =
                ConditionRoute::compile_for_behavior(&slot.conditions, &slot.behavior.spec);
            assert!(slot.compiled_subscriptions().is_ok_and(|keys| {
                keys.iter()
                    .any(|key| key.event == EventKind::ActionQueueSubmitted)
            }));
            runtime.catalog.insert(effect);
        }
        if !after_damage {
            let mut effect = runtime.catalog.get(31390122).unwrap().clone();
            let slot = &mut effect.slots[0];
            slot.conditions.clear();
            slot.compiled_route =
                ConditionRoute::compile_for_behavior(&slot.conditions, &slot.behavior.spec);
            runtime.catalog.insert(effect);
        }
        if held {
            grant_buff(&mut runtime, 13, 13, 31390121);
        }
        runtime.fight.cur_round = Some(1);
        runtime.round_state.act_point = 2 + i32::from(matching);
        let reply = runtime.plan_auto_round(&AutoRoundRequest::default());
        let positions = reply
            .opers
            .iter()
            .map(|oper| oper.param1)
            .collect::<Vec<_>>();
        let mut expected_positions = vec![Some(1), Some(expected)];
        if matching {
            expected_positions.insert(0, Some(1));
        }
        assert_eq!(
            positions, expected_positions,
            "matching={matching} passive={passive} held={held} conditional={conditional} after_damage={after_damage}",
        );
    }
}
