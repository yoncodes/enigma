use super::*;
use crate::engine::{
    manager::{
        buff::{BuffChangeDuration, BuffCommand, BuffGrant, BuffSelector},
        card::{CardCommand, CardOpType},
        conduit::ConduitCommand,
    },
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};
use sonettobuf::{AutoRoundRequest, CardInfo, FightDeviceAreaInfo, HeroAttribute};

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

fn change_buff_duration(runtime: &mut BattleRuntime, target_uid: i64, buff_id: i32, delta: i32) {
    runtime
        .managers
        .execute_buff(BuffCommand::ChangeDuration(BuffChangeDuration {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            target_uid,
            selector: BuffSelector::ExactId(buff_id),
            delta,
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
fn auto_round_plays_energy_cards_before_other_cards() {
    let runtime = team_runtime(
        vec![
            team_hero(10, 3066, 1_000, [30660111, 30660121]),
            team_hero(11, 3144, 1_000, [31446011, 31446012]),
        ],
        vec![
            hand_card(10, 30660111, false),
            hand_card(11, 31446011, false),
        ],
    );

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    let played: Vec<_> = reply.opers.iter().map(|oper| oper.param1).collect();
    assert_eq!(played, vec![Some(2)]);
}

#[test]
fn auto_round_loads_visible_conduit_card_roots() {
    let runtime = team_runtime(
        vec![team_hero(10, 3144, 1_000, [31440111, 31440121])],
        vec![hand_card(10, 31446011, false)],
    );

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers[0].param1, Some(1));
}

#[test]
fn auto_round_selects_conduit_groups_from_round_utility() {
    crate::test_support::init_config();
    let hero = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        team_type: Some(1),
        position: Some(uid as i32 - 9),
        current_hp: Some(1_000),
        attr: Some(HeroAttribute {
            hp: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut regulus = hero(13, 3025);
    regulus.destiny_stone = Some(302502);
    regulus.destiny_rank = Some(4);
    let fight = Fight {
        battle_id: Some(119383110),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![hero(10, 3149), hero(11, 3144), hero(12, 3145), regulus],
            device_area: Some(FightDeviceAreaInfo::default()),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                position: Some(1),
                career: Some(1),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    ..Default::default()
                }),
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
            hand: [
                (11, 31446012),
                (11, 31446011),
                (13, 31447002),
                (10, 31490201),
                (13, 31446021),
                (10, 31446012),
                (11, 31446011),
                (12, 31450181),
                (13, 31446021),
                (10, 31446022),
            ]
            .into_iter()
            .map(|(uid, skill_id)| hand_card(uid, skill_id, false))
            .collect(),
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    runtime.round_state.act_point = 5;

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(
        reply
            .opers
            .iter()
            .map(|oper| oper.param1)
            .collect::<Vec<_>>(),
        vec![Some(3), Some(9), Some(7), Some(4), Some(6)]
    );

    assert_eq!(
        reply
            .devices_opers
            .iter()
            .map(|oper| (oper.uid, oper.index))
            .collect::<Vec<_>>(),
        vec![
            (Some(10), Some(2)),
            (Some(11), Some(2)),
            (Some(12), Some(1)),
            (Some(13), Some(1)),
        ]
    );
}

#[test]
fn auto_round_does_not_report_a_future_card_driven_group_change() {
    let runtime = everecho_switch_runtime(vec![hand_card(10, 31450181, false)], 1);

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers[0].param1, Some(1));
    assert_eq!(reply.devices_opers[0].index, Some(1));
}

fn everecho_switch_runtime(hand: Vec<CardInfo>, act_point: i32) -> BattleRuntime {
    let mut runtime = team_runtime(
        vec![
            team_hero(10, 3145, 1_000, [31450111, 31450121]),
            team_hero(11, 3023, 500, [30230111, 30230121]),
        ],
        hand,
    );
    runtime.round_state.act_point = act_point;
    runtime
        .managers
        .conduit
        .execute(ConduitCommand::StopSkill {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60294, "StopDeviceSkill"),
            },
            source_uid: 10,
            team: 1,
            skill_id: 31450111,
        })
        .unwrap();
    runtime
}

#[test]
fn auto_round_does_not_feed_one_devices_switch_with_another_owners_energy() {
    let runtime = everecho_switch_runtime(
        vec![
            hand_card(11, 31446012, false),
            hand_card(10, 31450181, false),
        ],
        2,
    );

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers.len(), 2);
    assert_eq!(reply.opers[0].param1, Some(2));
    assert_eq!(reply.devices_opers[0].index, Some(1));
}

#[test]
fn auto_round_feeds_a_devices_switch_with_its_own_later_energy() {
    let runtime = everecho_switch_runtime(
        vec![
            hand_card(10, 31446062, false),
            hand_card(10, 31450181, false),
        ],
        2,
    );

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers.len(), 2);
    assert_eq!(reply.opers[0].param1, Some(2));
    assert_eq!(reply.devices_opers[0].index, Some(2));
}

#[test]
fn auto_round_multi_device_plan_executes_without_cross_feeding_switches() {
    let mut runtime = team_runtime(
        vec![
            team_hero(10, 3145, 1_000, [31450111, 31450121]),
            team_hero(11, 3149, 1_000, [31490111, 31490121]),
        ],
        vec![
            hand_card(10, 31450181, false),
            hand_card(11, 31446012, false),
        ],
    );
    runtime.round_state.act_point = 2;
    runtime
        .managers
        .conduit
        .execute(ConduitCommand::StopSkill {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60294, "StopDeviceSkill"),
            },
            source_uid: 10,
            team: 1,
            skill_id: 31450111,
        })
        .unwrap();

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());
    let everecho_group = reply
        .devices_opers
        .iter()
        .find(|operation| operation.uid == Some(10))
        .and_then(|operation| operation.index);
    let twins_group = reply
        .devices_opers
        .iter()
        .find(|operation| operation.uid == Some(11))
        .and_then(|operation| operation.index);
    assert_eq!(everecho_group, Some(1));
    for operation in &reply.devices_opers {
        runtime
            .managers
            .conduit
            .execute(ConduitCommand::SelectGroup {
                source_uid: operation.uid.unwrap(),
                group: operation.index.unwrap(),
            })
            .unwrap();
    }
    let pool = crate::engine::skill::target::TargetPool::from_fight_with_catalog(
        runtime.catalog_data.unwrap(),
        &runtime.fight,
    );
    let mut catalog = runtime.catalog.clone();
    runtime.managers.catalog().extend_skill_roots(
        &mut catalog,
        [31450181, 31446012],
        std::iter::empty(),
    );
    let mut determinism = runtime.determinism.clone();
    crate::engine::runtime::schedule::run_player_commands(
        &mut runtime.managers,
        &pool,
        &catalog,
        &mut determinism,
        crate::engine::skill::target::TargetContext::default(),
        reply
            .opers
            .iter()
            .filter_map(crate::engine::round::command::RoundCommand::from_oper),
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();

    assert_eq!(runtime.managers.conduit.selected_group(10), Some(2));
    assert_eq!(runtime.managers.conduit.selected_group(11), twins_group);
}

#[test]
fn auto_round_tracks_a_prefilled_conduit_switch_before_generated_energy() {
    let runtime = everecho_switch_runtime(
        vec![
            hand_card(11, 31446012, false),
            hand_card(10, 31450181, false),
        ],
        2,
    );
    let prefix = runtime.plan_auto_round(&AutoRoundRequest::default()).opers[0].clone();
    assert_eq!(prefix.param1, Some(2));

    let reply = runtime.plan_auto_round(&AutoRoundRequest {
        opers: vec![prefix],
        to_id: Some(-1),
    });

    assert_eq!(reply.opers.len(), 1);
    assert_eq!(reply.opers[0].param1, Some(1));
    assert_eq!(reply.devices_opers[0].index, Some(1));
}

#[test]
fn auto_round_selects_conduit_healing_only_for_urgent_hp() {
    for (ally_hp, expected_group) in [(700, 1), (500, 2)] {
        let mut runtime = team_runtime(
            vec![
                team_hero(10, 3145, 1_000, [31450111, 31450121]),
                team_hero(11, 3023, ally_hp, [30230111, 30230121]),
            ],
            vec![hand_card(10, 31446062, false)],
        );
        runtime
            .managers
            .conduit
            .execute(ConduitCommand::StopSkill {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(60294, "StopDeviceSkill"),
                },
                source_uid: 10,
                team: 1,
                skill_id: 31450111,
            })
            .unwrap();

        let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

        assert_eq!(
            reply.devices_opers[0].index,
            Some(expected_group),
            "ally hp {ally_hp}"
        );
    }
}

#[test]
fn auto_round_conduit_lookahead_respects_higher_priority_action_points() {
    let mut ally = team_hero(11, 3023, 1_000, [30230111, 30230121]);
    ally.ex_point = Some(5);
    ally.ex_point_max = Some(5);
    ally.ex_skill = Some(30230131);
    let runtime = coppelia_conduit_runtime(
        ally,
        vec![
            hand_card(11, 30230131, false),
            hand_card(10, 31446011, false),
            hand_card(10, 31446011, false),
        ],
        2,
    );

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers.len(), 2);
    assert_eq!(reply.opers[0].param1, Some(1));
    assert_eq!(reply.devices_opers[0].index, Some(2));
}

#[test]
fn auto_round_keeps_a_ready_conduit_attack_while_its_setup_buff_is_expiring() {
    let mut runtime = coppelia_conduit_runtime(
        team_hero(11, 3023, 1_000, [30230111, 30230121]),
        vec![
            hand_card(10, 31446011, false),
            hand_card(10, 31446011, false),
        ],
        2,
    );
    for target_uid in [10, 11] {
        change_buff_duration(&mut runtime, target_uid, 31440111, -2);
    }

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.devices_opers[0].index, Some(1));
}

#[test]
fn auto_round_refreshes_an_expiring_buff_with_an_ordinary_card() {
    let mut runtime = coppelia_conduit_runtime(
        team_hero(11, 3023, 1_000, [30230111, 30230121]),
        vec![
            hand_card(11, 30230111, false),
            hand_card(10, 31440121, false),
            hand_card(11, 30230111, false),
        ],
        2,
    );
    for target_uid in [10, 11] {
        change_buff_duration(&mut runtime, target_uid, 31440111, -2);
    }

    let reply = runtime.plan_auto_round(&AutoRoundRequest {
        opers: vec![BeginRoundOper {
            oper_type: Some(CardOpType::PlayCard.id()),
            param1: Some(1),
            to_id: Some(-1),
            ..Default::default()
        }],
        to_id: Some(-1),
    });

    assert_eq!(reply.opers[0].param1, Some(1));
}

#[test]
fn auto_round_conduit_lookahead_keeps_energy_order_after_prefix() {
    let runtime = coppelia_conduit_runtime(
        team_hero(11, 3023, 1_000, [30230111, 30230121]),
        vec![
            hand_card(10, 31446011, false),
            hand_card(10, 31446011, false),
            hand_card(11, 31446021, false),
        ],
        2,
    );
    let reply = runtime.plan_auto_round(&AutoRoundRequest {
        opers: vec![BeginRoundOper {
            oper_type: Some(CardOpType::PlayCard.id()),
            param1: Some(1),
            to_id: Some(-1),
            ..Default::default()
        }],
        to_id: Some(-1),
    });

    assert_eq!(reply.opers.len(), 1);
    assert_eq!(reply.opers[0].param1, Some(1));
    assert_eq!(reply.devices_opers[0].index, Some(1));
}

fn coppelia_conduit_runtime(
    ally: FightEntityInfo,
    hand: Vec<CardInfo>,
    act_point: i32,
) -> BattleRuntime {
    crate::test_support::init_config();
    let mut runtime = runtime(Fight {
        battle_id: Some(77),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3144),
                    team_type: Some(1),
                    position: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ally,
            ],
            device_area: Some(FightDeviceAreaInfo::default()),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                position: Some(1),
                current_hp: Some(100_000),
                attr: Some(HeroAttribute {
                    hp: Some(100_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    runtime
        .managers
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::SelectGroup {
                source_uid: 10,
                group: 2,
            },
        )
        .unwrap();
    grant_buff(&mut runtime, 10, 10, 31440111);
    grant_buff(&mut runtime, 10, 11, 31440111);
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand,
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    runtime.round_state.act_point = act_point;
    runtime
}

fn twins_conduit_runtime(
    enemy_hp: i32,
    enemy_career: i32,
    cost_reduction: bool,
    ex_point: i32,
) -> BattleRuntime {
    twins_conduit_runtime_with_enemy_hps(&[enemy_hp], enemy_career, cost_reduction, ex_point)
}

fn twins_conduit_runtime_with_enemy_hps(
    enemy_hps: &[i32],
    enemy_career: i32,
    cost_reduction: bool,
    ex_point: i32,
) -> BattleRuntime {
    crate::test_support::init_config();
    let twins = FightEntityInfo {
        uid: Some(10),
        model_id: Some(3149),
        team_type: Some(1),
        position: Some(1),
        current_hp: Some(1_000),
        ex_point: Some(ex_point),
        ex_point_type: Some(4),
        ex_point_max: Some(100),
        ex_skill: Some(0),
        attr: Some(HeroAttribute {
            hp: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = runtime(Fight {
        battle_id: Some(77),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![twins],
            device_area: Some(FightDeviceAreaInfo::default()),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: enemy_hps
                .iter()
                .enumerate()
                .map(|(index, hp)| FightEntityInfo {
                    uid: Some(-(index as i64) - 1),
                    team_type: Some(2),
                    position: Some(index as i32 + 1),
                    career: Some(enemy_career),
                    current_hp: Some(*hp),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    });
    grant_buff(&mut runtime, 10, 10, 31490006);
    grant_buff(&mut runtime, 10, 10, 31490007);
    if cost_reduction {
        grant_buff(&mut runtime, 10, 10, 31440113);
    }
    runtime
}

fn regulus_conduit_runtime(enemy_hp: i32) -> BattleRuntime {
    crate::test_support::init_config();
    runtime(Fight {
        battle_id: Some(77),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3025),
                team_type: Some(1),
                position: Some(1),
                ex_skill_level: Some(3),
                destiny_stone: Some(302502),
                destiny_rank: Some(4),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            device_area: Some(FightDeviceAreaInfo::default()),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                position: Some(1),
                current_hp: Some(enemy_hp),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    })
}

#[test]
fn auto_round_keeps_the_current_conduit_group_when_power_is_insufficient() {
    let insufficient = regulus_conduit_runtime(1_000).plan_auto_round(&AutoRoundRequest::default());
    assert_eq!(insufficient.devices_opers[0].index, Some(1));
}

#[test]
fn auto_round_conduit_groups_honor_reduced_costs() {
    let reduced =
        twins_conduit_runtime(1_000, 1, true, 0).plan_auto_round(&AutoRoundRequest::default());
    assert_eq!(reduced.devices_opers[0].index, Some(2));
}

#[test]
fn auto_round_selects_ready_conduit_unique_without_a_character_ultimate() {
    let reply =
        twins_conduit_runtime(1_000, 1, false, 100).plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.devices_opers[0].index, Some(3));
}

#[test]
fn auto_round_does_not_rank_a_self_setup_buff_as_ally_support() {
    let reply =
        twins_conduit_runtime(1_000, 0, false, 70).plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.devices_opers[0].index, Some(1));
}

#[test]
fn auto_round_feeds_the_selected_conduit_group_first() {
    let mut runtime = twins_conduit_runtime(1_000, 0, false, 70);
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: [31446012, 31446021, 31446011, 31446011, 31446022, 31446022]
                .into_iter()
                .map(|skill_id| hand_card(10, skill_id, false))
                .collect(),
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    runtime.round_state.act_point = 2;

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(
        reply
            .opers
            .iter()
            .map(|oper| oper.param1)
            .collect::<Vec<_>>(),
        vec![Some(1), Some(2)]
    );
    assert_eq!(reply.devices_opers[0].index, Some(1));
}

#[test]
fn auto_round_prefers_flexible_energy_when_equal_amounts_are_usable() {
    let mut runtime = twins_conduit_runtime(1_000, 0, false, 70);
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: [31446022, 31446062]
                .into_iter()
                .map(|skill_id| hand_card(10, skill_id, false))
                .collect(),
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    runtime.round_state.act_point = 1;

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers[0].param1, Some(2));
}

#[test]
fn auto_round_keeps_equal_special_energy_cards_in_hand_order() {
    let mut runtime = team_runtime(
        vec![
            team_hero(10, 3149, 1_000, [31490111, 31490121]),
            team_hero(11, 3145, 1_000, [31450111, 31450121]),
        ],
        vec![
            hand_card(10, 31490211, false),
            hand_card(11, 31450181, false),
        ],
    );
    runtime.round_state.act_point = 2;
    runtime
        .managers
        .conduit
        .execute(ConduitCommand::SelectGroup {
            source_uid: 10,
            group: 2,
        })
        .unwrap();
    for skill_id in [31490111, 31490121] {
        runtime
            .managers
            .conduit
            .execute(ConduitCommand::StopSkill {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(60294, "StopDeviceSkill"),
                },
                source_uid: 10,
                team: 1,
                skill_id,
            })
            .unwrap();
    }

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(
        reply
            .opers
            .iter()
            .map(|operation| operation.param1)
            .collect::<Vec<_>>(),
        vec![Some(1), Some(1)]
    );
}

#[test]
fn auto_round_plays_temporary_device_cards_with_normal_cost() {
    let mut runtime = twins_conduit_runtime(1_000, 1, true, 0);
    runtime
        .managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![
                hand_card(10, 31490211, true),
                hand_card(10, 31446022, false),
            ],
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    runtime.round_state.act_point = 1;

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers.len(), 1);
    assert_eq!(reply.opers[0].param1, Some(1));
}

#[test]
fn auto_round_prefers_an_owner_bound_temporary_skill() {
    let mut runtime = team_runtime(
        vec![
            team_hero(10, 3124, 1_000, [312431212, 312451115]),
            team_hero(11, 3123, 1_000, [312301142, 312301242]),
        ],
        vec![
            CardInfo {
                uid: Some(10),
                skill_id: Some(312451011),
                temp_card: Some(true),
                card_type: Some(sonettobuf::card_info::CardType::Skill3 as i32),
                hero_id: Some(3124),
                ..Default::default()
            },
            hand_card(11, 312301142, false),
        ],
    );
    runtime.round_state.act_point = 1;

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers[0].param1, Some(1));
}

#[test]
fn auto_round_keeps_equal_energy_cards_in_hand_order_after_an_owner_plays() {
    let mut ally = team_hero(11, 3023, 1_000, [30230111, 30230121]);
    ally.ex_point = Some(5);
    ally.ex_point_max = Some(5);
    ally.ex_skill = Some(30230131);
    let runtime = coppelia_conduit_runtime(
        ally,
        vec![
            hand_card(11, 30230131, false),
            hand_card(11, 31446021, false),
            hand_card(10, 31446021, false),
        ],
        2,
    );

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(
        reply
            .opers
            .iter()
            .map(|oper| oper.param1)
            .collect::<Vec<_>>(),
        vec![Some(1), Some(1)]
    );
}

#[test]
fn auto_round_selects_an_enemy_while_conduit_energy_resolves_on_its_owner() {
    let setup = |runtime: &mut BattleRuntime| {
        runtime
            .managers
            .execute_card(CardCommand::Setup(CardSetup {
                hand: vec![hand_card(10, 31446012, false)],
                draw_pile: Vec::new(),
                deck_num: 0,
            }))
            .unwrap();
        runtime.round_state.act_point = 1;
    };
    let mut runtime = twins_conduit_runtime_with_enemy_hps(&[1_000, 1_000], 0, false, 70);
    setup(&mut runtime);

    let reply = runtime.plan_auto_round(&AutoRoundRequest {
        to_id: Some(-2),
        ..Default::default()
    });

    assert_eq!(reply.opers[0].to_id, Some(-2));
    let pool = crate::engine::skill::target::TargetPool::from_fight_with_catalog(
        runtime.catalog_data.unwrap(),
        &runtime.fight,
    );
    let mut catalog = runtime.catalog.clone();
    runtime
        .managers
        .catalog()
        .extend_skill_roots(&mut catalog, [31446012], std::iter::empty());
    let mut determinism = runtime.determinism.clone();
    crate::engine::runtime::schedule::run_player_commands(
        &mut runtime.managers,
        &pool,
        &catalog,
        &mut determinism,
        crate::engine::skill::target::TargetContext::default(),
        reply
            .opers
            .iter()
            .filter_map(crate::engine::round::command::RoundCommand::from_oper),
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();
    assert_eq!(runtime.managers.conduit.power(1, 1), 2);

    let mut fallback = twins_conduit_runtime_with_enemy_hps(&[1_000, 0], 0, false, 70);
    setup(&mut fallback);

    let reply = fallback.plan_auto_round(&AutoRoundRequest {
        to_id: Some(-2),
        ..Default::default()
    });

    assert_eq!(reply.opers[0].to_id, Some(-1));
}

#[test]
fn auto_round_keeps_the_current_conduit_group_without_a_legal_target() {
    let reply = regulus_conduit_runtime(0).plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.devices_opers[0].index, Some(1));
}

#[test]
fn auto_round_gives_every_hero_a_card_before_repeating_one() {
    let mut runtime = team_runtime(
        vec![
            team_hero(10, 3066, 1_000, [30660111, 30660121]),
            team_hero(11, 3074, 1_000, [30740111, 30740121]),
        ],
        vec![
            hand_card(10, 30660111, false),
            hand_card(10, 30660121, false),
            hand_card(11, 30740111, false),
        ],
    );
    runtime.round_state.act_point = 2;

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    let played: Vec<_> = reply.opers.iter().map(|oper| oper.param1).collect();
    assert_eq!(played, vec![Some(1), Some(2)]);
}

#[test]
fn auto_round_puts_an_urgent_heal_before_other_heroes_first_cards() {
    for (ally_hp, expected) in [(500, (Some(1), Some(10))), (700, (Some(2), Some(-1)))] {
        let mut runtime = team_runtime(
            vec![
                team_hero(10, 3066, ally_hp, [30660111, 30660121]),
                team_hero(11, 3082, 1_000, [30820111, 30820121]),
            ],
            vec![
                hand_card(11, 30820111, false),
                hand_card(11, 30820121, false),
                hand_card(10, 30660111, false),
            ],
        );
        runtime.round_state.act_point = 2;
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

        assert_eq!(reply.opers.len(), 1, "ally hp {ally_hp}");
        assert_eq!(
            (reply.opers[0].param1, reply.opers[0].to_id),
            expected,
            "ally hp {ally_hp}"
        );
    }
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
