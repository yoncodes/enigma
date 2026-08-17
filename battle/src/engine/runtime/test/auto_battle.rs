use super::*;
use crate::engine::{
    manager::{
        buff::{BuffCommand, BuffGrant},
        card::{CardCommand, CardOpType},
    },
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};
use sonettobuf::{AutoRoundRequest, CardInfo};

fn auto_runtime() -> BattleRuntime {
    crate::test_support::init_config();
    let fight = Fight {
        battle_id: Some(77),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3023),
                team_type: Some(1),
                position: Some(1),
                current_hp: Some(1_000),
                ex_point: Some(0),
                ex_skill: Some(30230131),
                skill_group1: vec![30230111],
                skill_group2: vec![30230121],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    team_type: Some(2),
                    position: Some(1),
                    current_hp: Some(1_000),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    team_type: Some(2),
                    position: Some(2),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let card = |skill_id, temp_card| CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        temp_card: Some(temp_card),
        ..Default::default()
    };
    let mut runtime = runtime(fight);
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![
                card(30230111, false),
                card(30230121, false),
                card(30230111, true),
            ],
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    runtime.round_state.act_point = 1;
    runtime
}

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
fn auto_round_skips_an_ultimate_without_its_required_resource() {
    let mut runtime = auto_runtime();
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![
                CardInfo {
                    uid: Some(10),
                    skill_id: Some(30230131),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(10),
                    skill_id: Some(30230111),
                    ..Default::default()
                },
            ],
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers.len(), 1);
    assert_eq!(reply.opers[0].param1, Some(2));
    runtime
        .advance_round(BeginRoundRequest {
            opers: reply.opers,
            auto_oper: Some(true),
            ..Default::default()
        })
        .unwrap();
}

#[test]
fn auto_round_spends_no_action_points_for_all_bendith_owner_skills() {
    crate::test_support::init_config();
    let fight = Fight {
        battle_id: Some(77),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3146),
                    team_type: Some(1),
                    position: Some(1),
                    current_hp: Some(1_000),
                    ex_point: Some(5),
                    ex_skill: Some(31460131),
                    skill_group1: vec![31460111],
                    skill_group2: vec![31460121],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    model_id: Some(3023),
                    team_type: Some(1),
                    position: Some(2),
                    current_hp: Some(1_000),
                    ex_point: Some(0),
                    ex_skill: Some(30230131),
                    skill_group1: vec![30230111],
                    skill_group2: vec![30230121],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                position: Some(1),
                current_hp: Some(1_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = runtime(fight);
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![
                CardInfo {
                    uid: Some(10),
                    skill_id: Some(31460131),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(10),
                    skill_id: Some(31460111),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(11),
                    skill_id: Some(30230111),
                    ..Default::default()
                },
            ],
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    runtime.round_state.act_point = 1;
    runtime
        .managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60001, "AddBuff"),
            },
            source_uid: 10,
            target_uid: 10,
            buff_id: 31460133,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers.len(), 3);
    let mut hand = runtime.card_hand().to_vec();
    let chosen = reply
        .opers
        .iter()
        .map(|oper| {
            let index = oper.param1.unwrap() as usize - 1;
            hand.remove(index).skill_id.unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(chosen, vec![31460131, 31460111, 30230111]);
}
