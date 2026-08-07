use super::*;

#[test]
fn conduit_inherits_the_rounds_selected_enemy() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-3),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let commands = vec![
        crate::engine::round::command::RoundCommand::PlayCard {
            card_index: 0,
            target_uid: Some(-3),
            chosen_skill_id: None,
            recorded_skill: None,
        },
        crate::engine::round::command::RoundCommand::PlayCard {
            card_index: 1,
            target_uid: Some(10),
            chosen_skill_id: None,
            recorded_skill: None,
        },
        crate::engine::round::command::RoundCommand::UseAssistBoss {
            skill_id: 1,
            target_uid: Some(-1),
        },
    ];

    assert_eq!(
        crate::engine::runtime::round::selected_enemy_target(
            &commands,
            &crate::engine::skill::target::TargetPool::from_fight(&fight),
        ),
        Some(-1)
    );
}

#[test]
fn destination_begin_round_owns_the_round_transition_and_reply_buckets() {
    crate::test_support::init_config();
    let fight = Fight {
        cur_round: Some(1),
        version: Some(6),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(fight);

    let round = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();

    assert_eq!(round.cur_round, Some(2));
    assert_eq!(runtime.fight.cur_round, Some(2));
    assert!(round.fight_step.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Smallroundend as i32)
        })
    }));
    assert_eq!(
        round.next_round_begin_step[0].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Dealcard1 as i32)
    );
    assert_eq!(
        round
            .next_round_begin_step
            .last()
            .unwrap()
            .act_effect
            .iter()
            .map(|effect| effect.effect_type.unwrap())
            .collect::<Vec<_>>(),
        vec![
            sonettobuf::effect_type_enum::EffectType::Cardspush as i32,
            sonettobuf::effect_type_enum::EffectType::Carddecknum as i32,
        ]
    );
}

#[test]
fn opening_round_uses_action_point_buffs_applied_during_setup() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                current_hp: Some(100),
                passive_skill: vec![31490161],
                ex_point_type: Some(4),
                ex_point_max: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-2),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(fight);

    let round = runtime.start_round().unwrap();

    assert_eq!(round.act_point, Some(2));
    let reset = round
        .fight_step
        .iter()
        .find(|step| {
            step.act_effect.iter().any(|effect| {
                effect.effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Devicepowerclear as i32)
            })
        })
        .unwrap();
    assert!(matches!(
        reset.act_effect.as_slice(),
        [.., clear, restart]
            if clear.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Devicepowerclear as i32)
                && clear.target_id == Some(0)
                && restart.effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Devicestop as i32)
                && restart.target_id == Some(10)
                && restart.effect_num == Some(0)
    ));
}

#[test]
fn opening_round_collects_static_ap_rules_without_runtime_dispatch() {
    crate::test_support::init_config();
    let fight = Fight {
        battle_id: Some(1108),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(fight);

    let round = runtime.start_round().unwrap();

    assert_eq!(round.act_point, Some(2));
}

#[test]
fn round_modifier_with_output_keeps_its_setup_command() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(fight);
    runtime.extend_battle_rule_skills([crate::engine::fight::rules::OwnedBattleSkill {
        owner_uid: crate::engine::fight::rules::ATTACKER_SIDE_UID,
        skill_id: 1_182_004,
    }]);

    runtime
        .build_start_steps(CardSetup {
            hand: (1..=5)
                .map(|uid| CardInfo {
                    uid: Some(uid),
                    temp_card: Some(false),
                    ..Default::default()
                })
                .collect(),
            draw_pile: Vec::new(),
            deck_num: 0,
        })
        .unwrap();

    assert_eq!(runtime.managers.card.hand().len(), 5);
    assert_eq!(runtime.managers.card.hand_limit_bonus(), 0);
}

#[test]
fn begin_round_refills_any_normal_hand_deficit() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(6),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(1001),
                    current_hp: Some(100),
                    skill_group1: vec![101, 102, 103],
                    skill_group2: vec![201, 202, 203],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let card = |skill_id| CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        temp_card: Some(false),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(fight);
    runtime
        .managers
        .execute_card(crate::engine::manager::card::CardCommand::Setup(
            CardSetup {
                hand: vec![card(101), card(201), card(101), card(201), card(101)],
                draw_pile: vec![card(201), card(101)],
                deck_num: 2,
            },
        ))
        .unwrap();
    runtime.determinism.enqueue_card_draws(vec![card(201)]);

    let round = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest {
            opers: vec![BeginRoundOper {
                oper_type: Some(
                    crate::engine::manager::card::CardOpType::SimulateDissolveCard as i32,
                ),
                param1: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();

    assert_eq!(runtime.managers.card.normal_hand_len(), 5);
    assert!(!round.before_cards2.is_empty());
    assert!(!round.team_a_cards2.is_empty());
}

#[cfg(feature = "private-fixtures")]
#[test]
fn round_start_keeps_precast_above_normal_hand_capacity() {
    crate::test_support::init_config();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../battle_preview/fixtures/battles/battle6/StartDungeonReply.json");
    let mut value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fixture).unwrap()).unwrap();
    crate::preview::normalize_live_json(&mut value);
    let fight: Fight = serde_json::from_value(value["fight"].clone()).unwrap();
    let mut runtime = BattleRuntime::new(fight);

    runtime.start_round().unwrap();
    assert!(
        runtime
            .use_cloth_skill(UseClothSkillRequest {
                from_id: Some(248_988_163),
                to_id: Some(110),
                r#type: Some(ClothSkillType::SelectCrystal as i32),
                ..Default::default()
            })
            .is_some()
    );
    let mut request_value: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../battle_preview/fixtures/battles/battle6/BeginRoundRequest_1.json"),
        )
        .unwrap(),
    )
    .unwrap();
    crate::preview::normalize_live_json(&mut request_value);
    let request: BeginRoundRequest = serde_json::from_value(request_value).unwrap();
    let round = runtime.build_begin_round_from_schedule(&request).unwrap();
    let normal = runtime.managers.card.normal_hand_len();
    let precast = runtime
        .managers
        .card
        .hand()
        .iter()
        .filter(|card| card.temp_card.unwrap_or_default())
        .count();

    assert_eq!(normal, 8);
    assert_eq!(precast, 1);
    assert_eq!(runtime.managers.card.hand().len(), 9);
    assert_eq!(
        round
            .before_cards1
            .iter()
            .filter(|card| card.temp_card.unwrap_or_default())
            .count(),
        1
    );
    assert!(round.next_round_begin_step.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect.card_info_list.len() == 9
                && effect
                    .card_info_list
                    .iter()
                    .filter(|card| card.temp_card.unwrap_or_default())
                    .count()
                    == 1
        })
    }));
}

#[cfg(feature = "private-fixtures")]
#[test]
fn rank_three_emanation_updates_lingering_glow() {
    crate::test_support::init_config();
    let battle = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../battle_preview/fixtures/battles/battle6");
    let mut value: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(battle.join("StartDungeonReply.json")).unwrap(),
    )
    .unwrap();
    crate::preview::normalize_live_json(&mut value);
    let fight: Fight = serde_json::from_value(value["fight"].clone()).unwrap();
    let mut runtime = BattleRuntime::new(fight);
    runtime.start_round().unwrap();
    assert!(
        runtime
            .use_cloth_skill(UseClothSkillRequest {
                from_id: Some(248_988_163),
                to_id: Some(110),
                r#type: Some(ClothSkillType::SelectCrystal as i32),
                ..Default::default()
            })
            .is_some()
    );

    let request = |round| {
        let mut value: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(battle.join(format!("BeginRoundRequest_{round}.json")))
                .unwrap(),
        )
        .unwrap();
        crate::preview::normalize_live_json(&mut value);
        serde_json::from_value::<BeginRoundRequest>(value).unwrap()
    };
    runtime
        .build_begin_round_from_schedule(&request(1))
        .unwrap();
    let key = crate::engine::mechanic::lingering_glow::key(1);
    let gained_before = runtime
        .managers
        .gauge
        .accumulated_raw_value(key, i64::MAX, i32::MAX)
        .unwrap();
    let mut emanation = request(2);
    emanation.opers.truncate(1);
    let round = runtime.build_begin_round_from_schedule(&emanation).unwrap();
    let gained_after = runtime
        .managers
        .gauge
        .accumulated_raw_value(key, i64::MAX, i32::MAX)
        .unwrap();
    let json = serde_json::to_string(&round).unwrap();

    assert!(gained_after - gained_before >= 75_000);
    assert!(json.contains("\"actId\":31340163"));
    assert!(json.contains("\"actId\":1050"));
}

#[test]
fn begin_round_projects_the_canonical_hand_after_the_deal_composes() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(6),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    skill_group1: vec![100, 101, 102],
                    skill_group2: vec![200, 201, 202],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let card = |skill_id| CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        temp_card: Some(false),
        ..Default::default()
    };
    let original = vec![card(200), card(300), card(400), card(500), card(100)];
    let mut runtime = BattleRuntime::new(fight);
    runtime
        .managers
        .execute_card(crate::engine::manager::card::CardCommand::Setup(
            CardSetup {
                hand: original.clone(),
                draw_pile: vec![card(100), card(200)],
                deck_num: 2,
            },
        ))
        .unwrap();
    runtime
        .determinism
        .enqueue_card_draws(vec![card(100), card(200)]);

    let round = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest {
            opers: vec![BeginRoundOper {
                oper_type: Some(
                    crate::engine::manager::card::CardOpType::SimulateDissolveCard as i32,
                ),
                param1: Some(4),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    let skills = |cards: &[CardInfo]| {
        cards
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>()
    };

    assert_eq!(
        skills(&round.before_cards1),
        skills(runtime.managers.card.hand())
    );
    assert_eq!(
        skills(&round.team_a_cards1),
        skills(runtime.managers.card.team_cards())
    );
    assert_eq!(skills(&round.before_cards2), vec![200, 300, 400, 100]);
    assert_eq!(skills(&round.team_a_cards2), vec![100, 200]);
    assert_eq!(
        skills(runtime.managers.card.hand()),
        vec![200, 300, 400, 101, 200]
    );
    assert!(
        round
            .fight_step
            .iter()
            .any(|step| step.act_effect.iter().any(|effect| {
                effect.effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Cardscompose as i32)
            }))
    );
}

#[test]
fn opening_random_pool_never_contains_an_ultimate() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(4),
                ex_skill: Some(900),
                skill_group1: vec![100],
                skill_group2: vec![200],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(fight);
    runtime.managers.ex_point.add(10, 10, 1, 0);

    assert_eq!(
        super::start::available_player_cards(&runtime.fight)
            .into_iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![100, 200]
    );
}

#[test]
fn finished_round_does_not_promote_reserves_and_still_projects_the_next_round() {
    crate::test_support::init_config();
    let entity = |uid, hp, position| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(if uid > 0 { 1 } else { 2 }),
        current_hp: Some(hp),
        position: Some(position),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(Fight {
        cur_round: Some(1),
        version: Some(6),
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 100, 1)],
            sub_entitys: vec![entity(11, 100, -1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 100, 1)],
            ..Default::default()
        }),
        ..Default::default()
    });
    runtime.managers.hp.lose(10, 100, -1);

    let round = runtime
        .build_begin_round_from_schedule(&BeginRoundRequest::default())
        .unwrap();

    assert_eq!(round.is_finish, Some(true));
    assert!(!round.next_round_begin_step.is_empty());
    assert_eq!(
        runtime.fight.attacker.as_ref().unwrap().entitys[0].uid,
        Some(10)
    );
    assert_eq!(
        runtime.fight.attacker.as_ref().unwrap().sub_entitys[0].uid,
        Some(11)
    );
    assert!(round.fight_step.iter().all(|step| {
        step.act_effect.iter().all(|effect| {
            effect.effect_type != Some(sonettobuf::effect_type_enum::EffectType::Changehero as i32)
        })
    }));
}
