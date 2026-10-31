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
            vec![1, -1, 2],
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
        managers.catalog(),
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
    assert_eq!(steps[1].act_effect.len(), 2);
    assert!(steps[1].act_effect.iter().all(|effect| {
        effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Carddecknum as i32)
            && effect.effect_num == Some(30)
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
        managers.catalog(),
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
fn opening_setup_applies_the_configured_fourth_ally_limit_before_dealing() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: (0..4)
                .map(|index| FightEntityInfo {
                    uid: Some(index + 1),
                    position: Some(index as i32 + 1),
                    career: Some(if index == 3 { 101 } else { 6 }),
                    team_type: Some(1),
                    current_hp: Some(100),
                    passive_skill: (index == 3).then_some(40).into_iter().collect(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .battle_rule
        .extend_owned_skills([crate::engine::fight::rules::OwnedBattleSkill {
            owner_uid: crate::engine::fight::rules::ATTACKER_SIDE_UID,
            skill_id: 1163852001,
        }]);
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
    let mut catalog =
        SkillEffectCatalog::from_roots(config::configs::get(), [1163852001], std::iter::empty());
    catalog.insert(ParsedSkillEffect {
        skill_id: 40,
        slots: vec![slot],
    });
    let card = |skill_id| CardInfo {
        uid: Some(1),
        skill_id: Some(skill_id),
        ..Default::default()
    };
    let opening = [
        31070111, 31280121, 31070121, 31430121, 31430111, 31280111, 31070111, 31446011, 31446011,
        31070121, 31430111,
    ]
    .into_iter()
    .map(card)
    .collect::<Vec<_>>();
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_card_draws(opening[8..].to_vec());

    let (start, dealt) = run_start(
        managers.catalog(),
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
        CardSetup {
            hand: opening[..8].to_vec(),
            draw_pile: opening[8..].to_vec(),
            deck_num: 48,
        },
        8,
    )
    .unwrap();

    assert!(
        managers
            .buff
            .active_for(4)
            .any(|buff| buff.buff_id == Some(1163852002))
    );
    assert_eq!(
        crate::engine::mechanic::card::CardMechanic.normal_hand_limit(8, &managers, &pool),
        11
    );
    assert_eq!(dealt, opening);
    assert_eq!(managers.card.normal_hand_len(), 11);
    assert_eq!(managers.card.deck_num(), 46);
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
        vec![48, 46, 46]
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
        managers.catalog(),
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
        managers.catalog(),
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
        managers.catalog(),
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
        managers.catalog(),
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
fn version_seven_opening_orders_immunity_duration_and_buff_gate() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                passive_skill: vec![40, 31430171],
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31430144),
                    from_uid: Some(10),
                    act_info: vec![sonettobuf::BuffActInfo {
                        act_id: Some(1126),
                        param: vec![1],
                        str_param: Some(String::new()),
                    }],
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
    let mut enter_fight = SkillEffectSlot::new(
        ParsedBehavior::new(1, "AddBuff", vec![31280114, 1]),
        TargetRequest::self_only(),
    );
    enter_fight.conditions = vec![ParsedCondition {
        opcode: 5,
        type_name: "EnterFight".to_owned(),
        kind: ParsedConditionKind::Lifecycle(
            crate::engine::skill::condition::lifecycle::LifecycleMode::EnterFight,
        ),
        raw_args: Vec::new(),
    }];
    enter_fight.compiled_route = ConditionRoute::compile(&enter_fight.conditions);
    let mut catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31430171], []);
    catalog.insert(ParsedSkillEffect {
        skill_id: 40,
        slots: vec![enter_fight],
    });

    let (start, _) = run_start(
        managers.catalog(),
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

    let immunity_reset = start
        .outcomes
        .iter()
        .position(|outcome| {
            matches!(
                outcome,
                RuleOutcome::BuffActInfoMarker(marker)
                    if marker.target_uid == 10
                        && marker.buff_uid == 20
                        && marker.act_id == 1126
                        && marker.params == [4]
            )
        })
        .expect("opening resets the configured team immunity allowance");
    let duration_refresh = start
        .outcomes
        .iter()
        .position(|outcome| {
            matches!(
                outcome,
                RuleOutcome::BuffBatch(changes)
                    if changes.iter().any(|change| {
                        change.origin.key.opcode
                            == crate::engine::skill::buff_act::effect_time::ROUND_START_DURATION
                            && change.change.refreshed.iter().any(|refresh| {
                                refresh.after.buff_id == Some(31280114)
                                    && refresh.after.duration == Some(3)
                            })
                    })
            )
        })
        .expect("opening advances the pre-existing duration snapshot");
    let buff_gate = start
        .outcomes
        .iter()
        .position(|outcome| {
            matches!(
                outcome,
                RuleOutcome::Buff(changes)
                    if changes.change.added.iter().any(|added| {
                        added.buff.buff_id == Some(31430171)
                    })
            )
        })
        .expect("opening emits the configured BuffGate output");

    assert!(immunity_reset < duration_refresh && duration_refresh < buff_gate);
    assert_eq!(
        managers
            .buff
            .active_for(10)
            .find(|buff| buff.buff_id == Some(31280114))
            .and_then(|buff| buff.duration),
        Some(3)
    );
    assert_eq!(
        managers
            .buff
            .active_for(10)
            .find(|buff| buff.buff_id == Some(31430171))
            .and_then(|buff| buff.duration),
        Some(2)
    );
}

#[test]
fn version_seven_opening_interleaves_round_start_and_buff_gate_skills_by_owner() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    position: Some(1),
                    team_type: Some(1),
                    current_hp: Some(100),
                    passive_skill: vec![40],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    position: Some(2),
                    team_type: Some(1),
                    current_hp: Some(100),
                    passive_skill: vec![50],
                    buffs: vec![BuffInfo {
                        uid: Some(2),
                        buff_id: Some(30810305),
                        from_uid: Some(20),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(30),
                    position: Some(3),
                    team_type: Some(1),
                    current_hp: Some(100),
                    passive_skill: vec![60],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let slot = |condition| {
        let mut slot = SkillEffectSlot::new(
            ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
            TargetRequest::self_only(),
        );
        slot.conditions = vec![condition];
        slot.compiled_route = ConditionRoute::compile(&slot.conditions);
        slot
    };
    let round_start = || ParsedCondition {
        opcode: 103,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(NoneMode::RoundStart),
        raw_args: Vec::new(),
    };
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 40,
        slots: vec![slot(round_start())],
    });
    catalog.insert(ParsedSkillEffect {
        skill_id: 50,
        slots: vec![slot(ParsedCondition {
            opcode: 19103,
            type_name: "HasBuffId".to_owned(),
            kind: ParsedConditionKind::BuffId {
                mode: BuffConditionMode::Present,
                buff_ids: vec![30810305],
            },
            raw_args: vec!["30810305".to_owned()],
        })],
    });
    catalog.insert(ParsedSkillEffect {
        skill_id: 60,
        slots: vec![slot(round_start())],
    });

    let (start, _) = run_start(
        managers.catalog(),
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

    let target_uids = start
        .outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            RuleOutcome::ExPoint(crate::engine::manager::ex_point::ExPointChanges::Value {
                change,
                ..
            }) => Some(change.target_uid),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(target_uids, [10, 20, 30]);
}

#[test]
fn version_seven_opening_groups_sync_and_late_setup() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                passive_skill: vec![40, 50],
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31270413),
                    from_uid: Some(10),
                    duration: Some(3),
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
    let slot = |condition| {
        let mut slot = SkillEffectSlot::new(
            ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
            TargetRequest::self_only(),
        );
        slot.conditions = vec![condition];
        slot.compiled_route = ConditionRoute::compile(&slot.conditions);
        slot
    };
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 40,
        slots: vec![slot(ParsedCondition {
            opcode: 19104,
            type_name: "HasBuffId".to_owned(),
            kind: ParsedConditionKind::BuffId {
                mode: BuffConditionMode::Present,
                buff_ids: vec![31270413],
            },
            raw_args: vec!["31270413".to_owned()],
        })],
    });
    catalog.insert(ParsedSkillEffect {
        skill_id: 50,
        slots: vec![slot(ParsedCondition {
            opcode: 104,
            type_name: "None".to_owned(),
            kind: ParsedConditionKind::None(NoneMode::RoundStart),
            raw_args: Vec::new(),
        })],
    });

    let (start, _) = run_start(
        managers.catalog(),
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

    fn contains_skill(step: &sonettobuf::FightStep, skill_id: i32) -> bool {
        step.act_id == Some(skill_id)
            || step.act_effect.iter().any(|effect| {
                effect
                    .fight_step
                    .as_ref()
                    .is_some_and(|child| contains_skill(child, skill_id))
            })
    }
    let steps = crate::engine::packet::timeline::project(&start.frames).unwrap();
    let sync = steps
        .iter()
        .position(|step| contains_skill(step, 40))
        .unwrap();
    let late = steps
        .iter()
        .position(|step| contains_skill(step, 50))
        .unwrap();

    assert_eq!(sync, late);
    assert_eq!(
        steps[sync]
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
            .flat_map(|step| &step.act_effect)
            .filter_map(|effect| effect.fight_step.as_ref())
            .filter_map(|step| step.act_id)
            .collect::<Vec<_>>(),
        [40, 50]
    );
}

#[test]
fn lingering_glow_opening_grants_team_attributes_under_its_opening_owner() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3134),
                    team_type: Some(1),
                    current_hp: Some(100),
                    passive_skill: vec![31340141, 31340142, 31340143, 31340140],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);

    let (opening, _) = run_start(
        managers.catalog(),
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
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

    let granted = opening
        .outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            RuleOutcome::Buff(result) => result.change.added.as_ref(),
            _ => None,
        })
        .filter(|added| added.buff.buff_id == Some(31340007))
        .map(|added| added.target_uid)
        .collect::<Vec<_>>();
    assert_eq!(granted, [10, 11]);

    fn contains_attribute_grant(frame: &SemanticFrame) -> bool {
        frame.items.iter().any(|item| match item {
            FrameItem::Change(change) => matches!(
                change.as_ref(),
                BattleChange::Buff(result)
                    if result
                        .change
                        .added
                        .iter()
                        .any(|added| added.buff.buff_id == Some(31340007))
            ),
            FrameItem::Child(child) => contains_attribute_grant(child),
            FrameItem::Cue(_) => false,
        })
    }
    let settlement = opening
        .frames
        .iter()
        .find(|frame| {
            matches!(
                frame.owner,
                FrameOwner::RoundPhase(RoundPhase::RoundStartSettlement)
            )
        })
        .expect("opening has a round-start settlement owner");
    let owner = settlement.items.iter().find_map(|item| match item {
        FrameItem::Child(frame)
            if matches!(frame.owner, FrameOwner::SetupMechanic)
                && matches!(
                    frame.trigger,
                    FrameTrigger::Setup {
                        stage: SetupStage::OpeningBuffGate,
                        priority: 0,
                    }
                ) =>
        {
            Some(frame)
        }
        FrameItem::Change(_) | FrameItem::Cue(_) | FrameItem::Child(_) => None,
    });
    assert!(owner.is_some_and(|frame| contains_attribute_grant(frame)));
}
