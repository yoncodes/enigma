use super::*;

#[test]
fn opening_cards_exist_before_card_setup_rules_run() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),

                passive_skill: vec![100],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(
            BehaviorSpec::new(60189, "AddEnergyToCard"),
            vec![1, 2, 1],
            Vec::new(),
        ),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 106,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(NoneMode::CardSetup),
        raw_args: Vec::new(),
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![slot],
    });

    let (result, _) = run_start(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        CardSetup {
            hand: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(200),
                card_effect: Some(1),
                energy: Some(0),
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 30,
        },
        1,
    )
    .unwrap();

    let card_outcomes = result
        .outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            RuleOutcome::Card(changes) => Some(changes.as_ref()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        card_outcomes
            .iter()
            .map(|change| change.kind)
            .collect::<Vec<_>>(),
        vec![
            crate::engine::manager::card::CardChangeKind::Setup,
            crate::engine::manager::card::CardChangeKind::EnergyChanged,
            crate::engine::manager::card::CardChangeKind::Composed,
        ]
    );
    assert_eq!(managers.card.hand()[0].energy, Some(2));
    assert_eq!(managers.card.deck_num(), 30);
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert_eq!(
        steps.first().unwrap().act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Enterfightdeal as i32)
    );
    assert!(steps.iter().any(|step| {
        step.act_effect.len() == 2
            && step.act_effect.iter().all(|effect| {
                effect.effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Carddecknum as i32)
                    && effect.effect_num == Some(30)
            })
    }));
    assert_eq!(
        steps.last().unwrap().act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Carddecknum as i32)
    );
}

#[test]
fn opening_raw_deal_uses_surplus_cards_to_refill_composed_slots() {
    init_config();
    let entity = |uid, position, first, second| FightEntityInfo {
        uid: Some(uid),
        position: Some(position),
        team_type: Some(1),
        current_hp: Some(100),
        skill_group1: vec![first, first + 1, first + 2],
        skill_group2: vec![second, second + 1, second + 2],
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 100, 200), entity(20, 2, 300, 400)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let card = |uid, skill_id| CardInfo {
        uid: Some(uid),
        skill_id: Some(skill_id),
        ..Default::default()
    };
    let raw_deal = vec![
        card(10, 200),
        card(10, 200),
        card(10, 200),
        card(20, 300),
        card(20, 300),
        card(20, 400),
        card(10, 200),
    ];
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);

    let (_, dealt) = run_start(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        CardSetup {
            hand: raw_deal.clone(),
            draw_pile: Vec::new(),
            deck_num: 32,
        },
        5,
    )
    .unwrap();

    assert_eq!(dealt, raw_deal);
    assert_eq!(
        managers
            .card
            .refilled()
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![400, 200]
    );
    assert_eq!(managers.card.normal_hand_len(), 5);
    assert_eq!(
        managers
            .card
            .hand()
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![201, 200, 301, 400, 200]
    );
    assert_eq!(managers.card.deck_num(), 32);
}

#[test]
fn opening_setup_applies_the_active_draw_limit_before_dealing() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: (0..4)
                .map(|index| FightEntityInfo {
                    uid: Some(index + 1),
                    team_type: Some(1),
                    current_hp: Some(100),
                    passive_skill: (index == 0).then_some(40).into_iter().collect(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(1, "AddBuff", vec![31490001]),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 5,
        type_name: "EnterFight".to_owned(),
        kind: ParsedConditionKind::Lifecycle(
            crate::engine::skill::condition::lifecycle::LifecycleMode::EnterFight,
        ),
        raw_args: Vec::new(),
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 40,
        slots: vec![slot],
    });
    let card = |skill_id| CardInfo {
        uid: Some(1),
        skill_id: Some(skill_id),
        ..Default::default()
    };

    let (start, dealt) = run_start(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        CardSetup {
            hand: (1..=8).map(card).collect(),
            draw_pile: (9..=10)
                .flat_map(|skill_id| [card(skill_id), card(skill_id)])
                .collect(),
            deck_num: 64,
        },
        8,
    )
    .unwrap();

    assert_eq!(dealt.len(), 10);
    assert_eq!(managers.card.normal_hand_len(), 10);
    assert_eq!(managers.card.deck_num(), 62);
    let steps = crate::engine::packet::timeline::project(&start.frames).unwrap();
    assert_eq!(
        steps
            .iter()
            .flat_map(|step| &step.act_effect)
            .filter(|effect| {
                effect.effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Carddecknum as i32)
            })
            .filter_map(|effect| effect.effect_num)
            .collect::<Vec<_>>(),
        vec![64, 62, 62]
    );
}

#[test]
fn start_schedule_runs_the_leading_round_start_lane() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                passive_skill: vec![40],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 100,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(NoneMode::RoundStart),
        raw_args: Vec::new(),
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 40,
        slots: vec![slot],
    });

    run_start(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        CardSetup {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        0,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 1);
}

#[test]
fn opening_round_does_not_consume_a_timed_layered_buff() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3095),
                career: Some(5),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(30950113),
                    duration: Some(3),
                    layer: Some(8),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::default();

    run_start(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        CardSetup {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        0,
    )
    .unwrap();

    let buff = managers.buff.snapshot(10, 20).unwrap();
    assert_eq!(buff.duration, Some(3));
    assert_eq!(buff.layer, Some(8));
}

#[test]
fn opening_round_advances_a_timed_buff_granted_during_setup() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                passive_skill: vec![40],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(1, "AddBuff", vec![31280114, 1]),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 5,
        type_name: "EnterFight".to_owned(),
        kind: ParsedConditionKind::Lifecycle(
            crate::engine::skill::condition::lifecycle::LifecycleMode::EnterFight,
        ),
        raw_args: Vec::new(),
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 40,
        slots: vec![slot],
    });

    run_start(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        CardSetup {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        0,
    )
    .unwrap();

    let buff = managers
        .buff
        .active_for(10)
        .find(|buff| buff.buff_id == Some(31280114))
        .unwrap();
    assert_eq!(buff.duration, Some(3));
}

#[test]
fn opening_round_does_not_advance_a_buff_granted_by_round_start() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3095),
                career: Some(5),
                team_type: Some(1),
                current_hp: Some(100),
                passive_skill: vec![40],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(1, "AddBuff", vec![30950113, 8]),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 104,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(NoneMode::RoundStart),
        raw_args: Vec::new(),
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 40,
        slots: vec![slot],
    });

    let (start, _) = run_start(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        CardSetup {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        0,
    )
    .unwrap();

    let buff = managers
        .buff
        .active_for(10)
        .find(|buff| buff.buff_id == Some(30950113))
        .unwrap();
    assert_eq!(buff.duration, Some(3));
    let steps = crate::engine::packet::timeline::project(&start.frames).unwrap();
    assert!(!steps.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Buffupdate as i32)
                && effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(30950113)
        })
    }));
}

#[test]
fn start_schedule_finishes_unconditional_setup_before_round_start() {
    init_config();
    let unconditional = START
        .iter()
        .position(|step| *step == (SetupStage::Unconditional, 0))
        .unwrap();
    let first_round_start = START
        .iter()
        .position(|(stage, _)| *stage == SetupStage::RoundStart)
        .unwrap();

    assert!(unconditional < first_round_start);
    let sync = START
        .iter()
        .position(|step| *step == (SetupStage::BuffSync, 0))
        .unwrap();
    let late = START
        .iter()
        .position(|step| *step == (SetupStage::RoundStartLate, 0))
        .unwrap();
    let settlement = START
        .iter()
        .position(|step| *step == (SetupStage::RoundStart, 3))
        .unwrap();

    assert!(sync < late && late < settlement);
}

#[test]
fn early_round_start_precedes_condition_priorities() {
    assert_eq!(
        ROUND_START_BEFORE_DURATION_SETUP,
        &[
            (SetupStage::RoundStart, -1),
            (SetupStage::RoundStartCondition, 100),
            (SetupStage::RoundStartCondition, 101),
            (SetupStage::RoundStartCondition, 102),
        ]
    );
    let opening = opening_setup(7);
    assert!(
        opening
            .iter()
            .position(|step| *step == (SetupStage::RoundStartCondition, 102))
            < opening
                .iter()
                .position(|step| *step == (SetupStage::RoundStart, -1))
    );
}

#[test]
fn configured_special_temp_card_runs_during_the_opening_round_start_card_event() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3114),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(12),
                    buff_id: Some(31140143),
                    from_uid: Some(10),
                    layer: Some(1),
                    duration: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::default();
    let (start, _) = run_start(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 1,
            ..Default::default()
        },
        CardSetup {
            hand: (0..7)
                .map(|index| CardInfo {
                    uid: Some(10),
                    skill_id: Some(100 + index),
                    ..Default::default()
                })
                .collect(),
            draw_pile: Vec::new(),
            deck_num: 48,
        },
        7,
    )
    .unwrap();

    assert!(
        managers.card.hand().iter().any(|card| {
            card.skill_id == Some(31140151) && card.uid == Some(10) && card.temp_card == Some(true)
        }),
        "hand={:?} active_features={:?}",
        managers.card.hand(),
        managers.buff.active_features(&managers.hp),
    );
    assert!(!managers.buff.has_buff_id(10, 31140143));
    let steps = crate::engine::packet::timeline::project(&start.frames).unwrap();
    let temp = steps
        .iter()
        .flat_map(|step| &step.act_effect)
        .find_map(|effect| effect.fight_step.as_ref())
        .unwrap();
    assert_eq!(
        temp.act_effect
            .iter()
            .map(|effect| effect.effect_type)
            .collect::<Vec<_>>(),
        vec![
            Some(sonettobuf::effect_type_enum::EffectType::Spcardadd as i32),
            Some(sonettobuf::effect_type_enum::EffectType::Changetotempcard as i32),
        ]
    );
    assert_eq!(temp.act_effect[1].reserve_str.as_deref(), Some("8"));
    assert!(steps.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Buffdel as i32)
                && effect.target_id == Some(10)
                && effect.config_effect == Some(0)
                && effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(31140143)
                && effect.buff.as_ref().and_then(|buff| buff.layer) == Some(1)
        })
    }));
}

#[test]
fn opening_round_start_conditions_only_run_for_the_player_side() {
    init_config();
    let entity = |uid, team_type| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(100),
        passive_skill: vec![40],
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 101,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(NoneMode::RoundStart),
        raw_args: Vec::new(),
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 40,
        slots: vec![slot],
    });

    run_start(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        CardSetup {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        0,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 1);
    assert_eq!(managers.ex_point.get(-1), 0);
}

#[test]
fn opening_keeps_new_one_round_buffs_until_their_configured_duration_stage() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100),
                passive_skill: vec![109360023],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    run_start(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 1,
            ..Default::default()
        },
        CardSetup {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        0,
    )
    .unwrap();

    let buff = managers
        .buff
        .active_for(-1)
        .find(|buff| buff.buff_id == Some(109320106))
        .cloned()
        .expect("configured one-round buff remains after opening");
    assert_eq!(buff.duration, Some(1));

    let expired = managers.buff.advance_durations_for_snapshot(
        crate::engine::skill::buff_act::effect_time::ROUND_START_DURATION,
        &[-1],
        &[buff.uid.unwrap()],
    );
    assert_eq!(expired.len(), 1);
    assert!(!managers.buff.has_buff_id(-1, 109320106));
}

#[test]
fn configured_conduit_is_initialized_before_battle_start_rules() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                team_type: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let (start, _) = run_start(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        CardSetup {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        0,
    )
    .unwrap();

    let steps = crate::engine::packet::timeline::project(&start.frames).unwrap();
    let effect = &steps[0].act_effect[0];
    assert_eq!(
        effect.effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Initdevice as i32)
    );
    let device = &effect.device_area_info.as_ref().unwrap().devices[0];
    assert_eq!(device.uid, Some(10));
    assert_eq!(device.index, Some(1));
    assert_eq!(
        device.skills[0]
            .skills
            .iter()
            .map(|skill| (skill.skill_id, skill.cost_type, skill.cost_value))
            .collect::<Vec<_>>(),
        vec![
            (Some(31490111), Some(1), Some(0)),
            (Some(31490121), Some(1), Some(3)),
        ]
    );
}
