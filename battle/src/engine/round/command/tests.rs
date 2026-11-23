use super::*;

#[test]
fn extracts_play_card_commands_from_begin_round_opers() {
    let opers = vec![
        BeginRoundOper {
            oper_type: Some(1),
            param1: Some(1),
            param2: Some(2),
            to_id: Some(0),
            ..Default::default()
        },
        BeginRoundOper {
            oper_type: Some(2),
            param1: Some(3),
            to_id: Some(-1),
            param3: Some(99),
            ..Default::default()
        },
        BeginRoundOper {
            oper_type: Some(3),
            param1: Some(2),
            param2: Some(3),
            ..Default::default()
        },
        BeginRoundOper {
            oper_type: Some(-99),
            param1: Some(1),
            ..Default::default()
        },
        BeginRoundOper {
            oper_type: Some(4),
            param1: Some(31_300_001),
            to_id: Some(-1),
            ..Default::default()
        },
    ];

    assert_eq!(
        commands_from_opers(&opers),
        vec![
            RoundCommand::MoveCard {
                from_index: 0,
                to_index: 1,
            },
            RoundCommand::PlayCard {
                card_index: 2,
                target_uid: Some(-1),
                chosen_skill_id: Some(99),
                recorded_skill: None,
            },
            RoundCommand::UseUniversal {
                universal_index: 1,
                target_index: 2,
            },
            RoundCommand::DissolveCard { card_index: 0 },
            RoundCommand::UseAssistBoss {
                skill_id: 31_300_001,
                target_uid: Some(-1),
            },
        ]
    );
}
