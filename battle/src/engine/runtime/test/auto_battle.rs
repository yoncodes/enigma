use super::*;
use crate::engine::{
    manager::{
        buff::{BuffCommand, BuffGrant},
        card::{CardCommand, CardOpType},
    },
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};
use sonettobuf::{AutoRoundRequest, CardInfo, HeroAttribute};

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
fn auto_round_casts_a_ready_ultimate_before_other_cards() {
    crate::test_support::init_config();
    let fight = Fight {
        battle_id: Some(77),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3066),
                team_type: Some(1),
                position: Some(1),
                current_hp: Some(1_000),
                ex_point: Some(0),
                ex_skill: Some(30660131),
                skill_group1: vec![30660111],
                skill_group2: vec![30660121],
                ..Default::default()
            }],
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
    runtime.round_state.act_point = 1;
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: [30660111, 30660121, 30660131]
                .map(|skill_id| CardInfo {
                    uid: Some(10),
                    skill_id: Some(skill_id),
                    ..Default::default()
                })
                .to_vec(),
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    runtime.managers.ex_point.set(10, 10, 5, 0);

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers.len(), 1);
    assert_eq!(reply.opers[0].param1, Some(3));
    runtime
        .advance_round(BeginRoundRequest {
            opers: reply.opers,
            auto_oper: Some(true),
            ..Default::default()
        })
        .unwrap();
}

fn team_hero(uid: i64, model_id: i32, hp: i32, skills: [i32; 2]) -> FightEntityInfo {
    FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        team_type: Some(1),
        position: Some(uid as i32 - 9),
        current_hp: Some(hp),
        attr: Some(HeroAttribute {
            hp: Some(1_000),
            ..Default::default()
        }),
        ex_point: Some(0),
        skill_group1: vec![skills[0]],
        skill_group2: vec![skills[1]],
        ..Default::default()
    }
}

fn support_runtime(healer_hp: i32, hand: Vec<CardInfo>) -> BattleRuntime {
    team_runtime(
        vec![
            team_hero(10, 3023, 1_000, [30230111, 30230121]),
            team_hero(11, 3082, healer_hp, [30820111, 30820121]),
        ],
        hand,
    )
}

fn team_runtime(team: Vec<FightEntityInfo>, hand: Vec<CardInfo>) -> BattleRuntime {
    team_runtime_against(team, vec![enemy(-1, 1_000)], hand)
}

fn enemy(uid: i64, hp: i32) -> FightEntityInfo {
    FightEntityInfo {
        uid: Some(uid),
        team_type: Some(2),
        position: Some(-uid as i32),
        current_hp: Some(hp),
        ..Default::default()
    }
}

fn team_runtime_against(
    team: Vec<FightEntityInfo>,
    enemies: Vec<FightEntityInfo>,
    hand: Vec<CardInfo>,
) -> BattleRuntime {
    crate::test_support::init_config();
    let fight = Fight {
        battle_id: Some(77),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: team,
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: enemies,
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = runtime(fight);
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand,
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    runtime.round_state.act_point = 1;
    runtime
}

fn grant_buff(runtime: &mut BattleRuntime, source_uid: i64, target_uid: i64, buff_id: i32) {
    runtime
        .managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid,
            target_uid,
            buff_id,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();
}

fn hand_card(uid: i64, skill_id: i32, temp_card: bool) -> CardInfo {
    CardInfo {
        uid: Some(uid),
        skill_id: Some(skill_id),
        temp_card: Some(temp_card),
        ..Default::default()
    }
}

#[test]
fn auto_round_heals_an_ally_below_eighty_percent_before_attacking() {
    let hand = || {
        vec![
            hand_card(11, 30820111, false),
            hand_card(11, 30820121, false),
        ]
    };

    let hurt = support_runtime(500, hand());
    let reply = hurt.plan_auto_round(&AutoRoundRequest::default());
    assert_eq!(reply.opers.len(), 1);
    assert_eq!(
        (reply.opers[0].param1, reply.opers[0].to_id),
        (Some(2), Some(11))
    );

    let healthy = support_runtime(1_000, hand());
    let reply = healthy.plan_auto_round(&AutoRoundRequest::default());
    assert_eq!(reply.opers[0].param1, Some(1));
}

#[test]
fn auto_round_sees_a_planned_heal_before_choosing_the_next_card() {
    let mut healer = team_hero(11, 3082, 1_000, [30820111, 30820121]);
    healer.attr = Some(HeroAttribute {
        hp: Some(1_000),
        attack: Some(1_000),
        ..Default::default()
    });
    let mut runtime = team_runtime(
        vec![team_hero(10, 3023, 790, [30230111, 30230121]), healer],
        vec![
            hand_card(11, 30820121, false),
            hand_card(10, 30230111, false),
            hand_card(11, 30820121, false),
        ],
    );
    runtime.round_state.act_point = 2;

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    let played: Vec<_> = reply
        .opers
        .iter()
        .map(|oper| (oper.param1, oper.to_id))
        .collect();
    assert_eq!(played, vec![(Some(1), Some(10)), (Some(1), Some(-1))]);
}

#[test]
fn auto_round_indices_replay_through_the_real_round_after_merges() {
    let mut runtime = team_runtime(
        vec![team_hero(10, 3023, 1_000, [30230111, 30230121])],
        vec![
            hand_card(10, 30230111, false),
            hand_card(10, 30230121, false),
            hand_card(10, 30230111, false),
            hand_card(10, 30230121, false),
        ],
    );
    runtime.round_state.act_point = 3;

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    let played: Vec<_> = reply
        .opers
        .iter()
        .map(|oper| (oper.param1, oper.to_id))
        .collect();
    assert_eq!(
        played,
        vec![
            (Some(1), Some(-1)),
            (Some(2), Some(-1)),
            (Some(1), Some(10))
        ]
    );
    runtime
        .advance_round(BeginRoundRequest {
            opers: reply.opers,
            auto_oper: Some(true),
            ..Default::default()
        })
        .unwrap();
}

#[test]
fn auto_round_picks_one_of_a_choice_cards_options() {
    let runtime_with_seed = |seed| {
        let mut runtime = team_runtime(
            vec![team_hero(10, 3120, 1_000, [31200111, 312001215])],
            vec![hand_card(10, 312001215, false)],
        );
        runtime.determinism = RoundDeterminism::with_seed(seed);
        runtime
    };
    let picks: std::collections::HashSet<_> = (1..=20)
        .map(|seed| {
            runtime_with_seed(seed)
                .plan_auto_round(&AutoRoundRequest::default())
                .opers[0]
                .param3
        })
        .collect();
    assert_eq!(picks, [Some(31200164), Some(31200231)].into());

    let mut runtime = runtime_with_seed(1);
    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());
    assert_eq!(reply.opers.len(), 1);
    runtime
        .advance_round(BeginRoundRequest {
            opers: reply.opers,
            auto_oper: Some(true),
            ..Default::default()
        })
        .unwrap();
}

#[test]
fn auto_round_plays_hero_precasts_first_and_leaves_stage_cards_alone() {
    let runtime = support_runtime(
        1_000,
        vec![
            hand_card(10, 30230111, false),
            hand_card(10, 30820111, true),
            hand_card(10, 30230121, true),
        ],
    );

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    let played: Vec<_> = reply.opers.iter().map(|oper| oper.param1).collect();
    assert_eq!(played, vec![Some(3), Some(1)]);
}

#[test]
fn auto_round_casts_ready_ultimates_left_to_right() {
    let mut thirty_seven = team_hero(10, 3066, 1_000, [30660111, 30660121]);
    thirty_seven.ex_skill = Some(30660131);
    let mut tooth_fairy = team_hero(11, 3053, 1_000, [30530111, 30530121]);
    tooth_fairy.ex_skill = Some(30530131);
    let mut runtime = team_runtime(
        vec![thirty_seven, tooth_fairy],
        vec![
            hand_card(10, 30660131, false),
            hand_card(11, 30530131, false),
        ],
    );
    runtime.managers.ex_point.set(10, 10, 5, 0);
    runtime.managers.ex_point.set(11, 11, 5, 0);

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers[0].param1, Some(1));
}

#[test]
fn auto_round_skips_buffs_every_target_already_holds() {
    let team = || {
        vec![
            team_hero(10, 3091, 1_000, [30910111, 30910121]),
            team_hero(11, 3072, 500, [30720111, 30720121]),
        ]
    };
    let grant = |runtime: &mut BattleRuntime, target_uid, buff_ids: &[i32]| {
        for &buff_id in buff_ids {
            grant_buff(runtime, 10, target_uid, buff_id);
        }
    };

    let mut runtime = team_runtime(
        vec![
            team_hero(10, 3106, 1_000, [31060111, 31060121]),
            team_hero(11, 3072, 500, [30720111, 30720121]),
        ],
        vec![hand_card(10, 31060121, false)],
    );
    grant(&mut runtime, 11, &[31060002]);
    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());
    assert_eq!(reply.opers[0].to_id, Some(10));

    let lorelei = [30910111, 30910121];
    let mut runtime = team_runtime(
        team(),
        vec![
            hand_card(10, 30910121, false),
            hand_card(11, 30720121, false),
        ],
    );
    grant(&mut runtime, 10, &lorelei);
    grant(&mut runtime, 11, &lorelei);
    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());
    assert_eq!(reply.opers[0].param1, Some(2));
}

#[test]
fn auto_round_plays_cards_made_free_by_bendith_first() {
    let mut runtime = team_runtime(
        vec![
            team_hero(10, 3146, 1_000, [31460111, 31460121]),
            team_hero(11, 3066, 1_000, [30660111, 30660121]),
        ],
        vec![
            hand_card(11, 30660111, false),
            hand_card(10, 31460121, false),
        ],
    );
    grant_buff(&mut runtime, 10, 10, 31460133);

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers[0].param1, Some(2));
}

#[test]
fn auto_round_never_puts_debuff_cards_last() {
    let mut runtime = team_runtime(
        vec![
            team_hero(10, 3098, 1_000, [30980111, 30980121]),
            team_hero(11, 3091, 1_000, [30910111, 30910121]),
        ],
        vec![
            hand_card(10, 30980121, false),
            hand_card(11, 30910121, false),
        ],
    );
    grant_buff(&mut runtime, 10, -1, 30980121);

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers[0].param1, Some(1));
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

#[test]
fn auto_round_ignores_conditional_buffs_when_judging_redundancy() {
    let mut runtime = team_runtime(
        vec![
            team_hero(10, 3087, 1_000, [30870111, 30870121]),
            team_hero(11, 3091, 1_000, [30910111, 30910121]),
        ],
        vec![
            hand_card(10, 30870111, false),
            hand_card(11, 30910121, false),
        ],
    );
    for target_uid in [10, 11] {
        for buff_id in [30870111, 30870121] {
            grant_buff(&mut runtime, 10, target_uid, buff_id);
        }
    }

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers[0].param1, Some(1));
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
