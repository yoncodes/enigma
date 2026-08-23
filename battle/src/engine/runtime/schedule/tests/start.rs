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
        31070111, 31280121, 31070121, 31430121, 31430111, 31280111, 31446011, 31446011, 31070111,
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
    assert_eq!(managers.card.deck_num(), 48);
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
        vec![48, 48, 48]
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

    assert!(!START.contains(&(SetupStage::EnterBattleStatic, 0)));
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

fn collect_effects_of_type<'a>(
    effect: &'a sonettobuf::ActEffect,
    effect_type: i32,
    matches: &mut Vec<&'a sonettobuf::ActEffect>,
) {
    if effect.effect_type == Some(effect_type) {
        matches.push(effect);
    }
    if let Some(step) = &effect.fight_step {
        for nested in &step.act_effect {
            collect_effects_of_type(nested, effect_type, matches);
        }
    }
}

fn step_contains_effect(step: &sonettobuf::FightStep, effect_type: i32) -> bool {
    let mut matches = Vec::new();
    for effect in &step.act_effect {
        collect_effects_of_type(effect, effect_type, &mut matches);
    }
    !matches.is_empty()
}

#[test]
fn anjo_contract_is_offered_once_during_opening_and_not_on_later_rounds() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3100),
                    position: Some(1),
                    team_type: Some(1),
                    current_hp: Some(100),
                    passive_skill: vec![31000141],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    model_id: Some(3086),
                    position: Some(2),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);
    let subscribers = crate::engine::event::dispatcher::dispatch_compiled_setup(
        &pool,
        &managers,
        &catalog,
        SetupStage::EnterBattleStatic,
        0,
    )
    .unwrap();
    assert_eq!(subscribers.len(), 1);
    assert_eq!(subscribers[0].0.owner_uid, 10);
    assert_eq!(subscribers[0].0.skill_id, 31000141);
    let mut determinism = RoundDeterminism::default();
    let (opening, _) = run_start(
        managers.catalog(),
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
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

    let contract_type = sonettobuf::effect_type_enum::EffectType::Notifiyherocontract as i32;
    let opening_steps =
        crate::engine::packet::timeline::project_for_version(&opening.frames, 7).unwrap();
    let mut opening_contracts = Vec::new();
    for effect in opening_steps.iter().flat_map(|step| &step.act_effect) {
        collect_effects_of_type(effect, contract_type, &mut opening_contracts);
    }
    assert_eq!(opening_contracts.len(), 1);
    assert_eq!(opening_contracts[0].target_id, Some(10));
    assert_eq!(opening_contracts[0].config_effect, Some(60092));
    assert_eq!(opening_contracts[0].reserve_str.as_deref(), Some("20"));
    assert!(managers.contract.selection_origin(10, 20).is_some());
    let enter_fight_deal = sonettobuf::effect_type_enum::EffectType::Enterfightdeal as i32;
    let deck_count = sonettobuf::effect_type_enum::EffectType::Carddecknum as i32;
    let enter_fight_deal = opening_steps
        .iter()
        .position(|step| step_contains_effect(step, enter_fight_deal))
        .unwrap();
    let deck_counts = opening_steps
        .iter()
        .enumerate()
        .filter(|(_, step)| step_contains_effect(step, deck_count))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let contract = opening_steps
        .iter()
        .position(|step| step_contains_effect(step, contract_type))
        .unwrap();
    assert_eq!(deck_counts.len(), 2);
    assert_eq!(
        opening_steps[deck_counts[0]]
            .act_effect
            .iter()
            .filter(|effect| effect.effect_type == Some(deck_count))
            .count(),
        2
    );
    assert_eq!(
        opening_steps[deck_counts[1]]
            .act_effect
            .iter()
            .filter(|effect| effect.effect_type == Some(deck_count))
            .count(),
        1
    );
    assert!(enter_fight_deal < deck_counts[0]);
    assert!(deck_counts[0] < contract && contract < deck_counts[1]);

    for round in [2, 3] {
        let (transition, _) = run_round_start_split(
            &mut managers,
            &pool,
            &catalog,
            &mut determinism,
            TargetContext {
                current_round: round,
                ..Default::default()
            },
            1,
        )
        .unwrap();
        let steps =
            crate::engine::packet::timeline::project_for_version(&transition.frames, 7).unwrap();
        let mut contracts = Vec::new();
        for effect in steps.iter().flat_map(|step| &step.act_effect) {
            collect_effects_of_type(effect, contract_type, &mut contracts);
        }
        assert!(
            contracts.is_empty(),
            "round {round} repeated the entry-only Contract offer"
        );
        assert!(managers.contract.selection_origin(10, 20).is_some());
    }
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
fn buff_gated_skill_rule_runs_during_the_opening_round_start_card_event() {
    init_config();
    let (fight, catalog) = buff_gated_generic_temp_card_fixture();
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
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
        1,
    )
    .unwrap();

    assert_buff_gated_generic_temp_card(&managers, &start);
}

#[test]
fn configured_hero_temp_card_uses_the_live_group_rank_and_projection() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3070),
                team_type: Some(1),
                current_hp: Some(100),
                skill_group1: vec![307001172, 307001182, 307001192],
                buffs: vec![BuffInfo {
                    uid: Some(1444),
                    buff_id: Some(307002612),
                    from_uid: Some(10),
                    duration: Some(0),
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
    let (start, _) = run_start(
        managers.catalog(),
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 4,
            ..Default::default()
        },
        CardSetup {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 48,
        },
        7,
    )
    .unwrap();

    assert!(managers.card.hand().iter().any(|card| {
        card.skill_id == Some(307001182) && card.uid == Some(10) && card.temp_card == Some(true)
    }));
    assert!(!managers.buff.has_buff_id(10, 307002612));

    fn collect_card_effects<'a>(
        effect: &'a sonettobuf::ActEffect,
        effects: &mut Vec<&'a sonettobuf::ActEffect>,
    ) {
        if matches!(
            effect.effect_type,
            Some(
                value
            ) if value == sonettobuf::effect_type_enum::EffectType::Spcardadd as i32
                || value
                    == sonettobuf::effect_type_enum::EffectType::Changetotempcard as i32
        ) {
            effects.push(effect);
        }
        if let Some(step) = &effect.fight_step {
            for nested in &step.act_effect {
                collect_card_effects(nested, effects);
            }
        }
    }
    let steps = crate::engine::packet::timeline::project(&start.frames).unwrap();
    let mut effects = Vec::new();
    for effect in steps.iter().flat_map(|step| &step.act_effect) {
        collect_card_effects(effect, &mut effects);
    }
    assert_eq!(effects.len(), 1);
    let effect = effects[0];
    assert_eq!(
        effect.effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Spcardadd as i32)
    );
    assert_eq!(effect.target_id, Some(10));
    assert_eq!(effect.team_type, Some(1));
    assert!(effect.card_info.as_ref().is_some_and(|card| {
        card.skill_id == Some(307001182)
            && card.temp_card == Some(true)
            && card.hero_id == Some(3070)
    }));
}

#[test]
fn configured_skill3_buff_adds_the_captured_card_and_removes_its_carrier() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3124),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(1622),
                    buff_id: Some(312451467),
                    from_uid: Some(10),
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
    let (start, _) = run_start(
        managers.catalog(),
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 2,
            ..Default::default()
        },
        CardSetup {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 48,
        },
        7,
    )
    .unwrap();

    let card = managers
        .card
        .hand()
        .iter()
        .find(|card| card.skill_id == Some(312451036))
        .expect("configured Skill 3 card is added to the hand");
    assert_eq!(card.uid, Some(10));
    assert_eq!(card.hero_id, Some(3124));
    assert_eq!(card.temp_card, Some(true));
    assert_eq!(
        card.card_type,
        Some(sonettobuf::card_info::CardType::Skill3 as i32)
    );
    assert!(!managers.buff.has_buff_id(10, 312451467));

    fn captured_step(effect: &sonettobuf::ActEffect) -> Option<&sonettobuf::FightStep> {
        let step = effect.fight_step.as_ref()?;
        if step.act_id == Some(312451467)
            && step.from_id == Some(10)
            && step.to_id == Some(10)
            && step.act_effect.iter().any(|effect| {
                effect.effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Addhandcard as i32)
            })
        {
            return Some(step);
        }
        step.act_effect.iter().find_map(captured_step)
    }
    let steps = crate::engine::packet::timeline::project(&start.frames).unwrap();
    let nested = steps
        .iter()
        .flat_map(|step| &step.act_effect)
        .find_map(captured_step)
        .expect("configured Skill 3 uses the carrier-owned nested frame");
    let card_effect = nested
        .act_effect
        .iter()
        .find(|effect| {
            effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Addhandcard as i32)
        })
        .unwrap();
    assert_eq!(card_effect.target_id, Some(10));
    assert_eq!(card_effect.effect_num, Some(0));
    assert_eq!(card_effect.reserve_id, Some(0));
    assert_eq!(card_effect.team_type, Some(1));
    assert!(card_effect.card_info_list.is_empty());
    assert!(card_effect.card_info.as_ref().is_some_and(|card| {
        card.uid == Some(10)
            && card.skill_id == Some(312451036)
            && card.hero_id == Some(3124)
            && card.temp_card == Some(true)
            && card.card_type == Some(sonettobuf::card_info::CardType::Skill3 as i32)
    }));
    assert!(steps.iter().any(|step| {
        step.act_effect.iter().any(|effect| {
            effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Buffdel as i32)
                && effect.target_id == Some(10)
                && effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(312451467)
                && effect.buff.as_ref().and_then(|buff| buff.uid) == Some(1622)
        })
    }));
}

#[test]
fn twins_round_start_passive_adds_the_captured_precast_card() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                team_type: Some(1),
                current_hp: Some(100),
                passive_skill: vec![116385685],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [116385685], []);
    assert!(catalog.get(116385685).is_some());
    assert!(
        !crate::engine::event::dispatcher::dispatch_compiled_setup(
            &pool,
            &managers,
            &catalog,
            SetupStage::RoundStartCondition,
            101,
        )
        .unwrap()
        .is_empty()
    );
    let start = crate::engine::runtime::drain::run_setup_stage(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 1,
            ..Default::default()
        },
        SetupStage::RoundStartCondition,
        101,
    )
    .unwrap();

    assert!(managers.card.hand().iter().any(|card| {
        card.skill_id == Some(31446013) && card.uid == Some(10) && card.temp_card == Some(true)
    }));
    fn has_precast(effect: &sonettobuf::ActEffect) -> bool {
        (effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Spcardadd as i32)
            && effect.target_id == Some(10)
            && effect.effect_num == Some(31446013)
            && effect.reserve_id == Some(3149))
            || effect
                .fight_step
                .as_ref()
                .is_some_and(|step| step.act_effect.iter().any(has_precast))
    }
    let steps = crate::engine::packet::timeline::project(&start.frames).unwrap();
    assert!(
        steps
            .iter()
            .flat_map(|step| &step.act_effect)
            .any(has_precast)
    );
}

#[test]
fn side_owned_round_start_skill_only_runs_for_the_scheduled_side() {
    init_config();
    let entity = |uid, team_type| FightEntityInfo {
        uid: Some(uid),
        position: Some(1),
        team_type: Some(team_type),
        current_hp: Some(100),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
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
    managers.battle_rule.extend_owned_skills([
        crate::engine::fight::rules::OwnedBattleSkill {
            owner_uid: crate::engine::fight::rules::ATTACKER_SIDE_UID,
            skill_id: 40,
        },
        crate::engine::fight::rules::OwnedBattleSkill {
            owner_uid: crate::engine::fight::rules::DEFENDER_SIDE_UID,
            skill_id: 41,
        },
    ]);
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(1, "AddBuff", vec![31490001]),
        TargetRequest {
            code: 124,
            raw: Vec::new(),
        },
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 101,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(NoneMode::RoundStart),
        raw_args: Vec::new(),
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    for skill_id in [40, 41] {
        catalog.insert(ParsedSkillEffect {
            skill_id,
            slots: vec![slot.clone()],
        });
    }

    run_start(
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

    assert!(managers.buff.has_buff_id(10, 31490001));
    assert!(!managers.buff.has_buff_id(-1, 31490001));
}

#[test]
fn twins_battle_rule_unlocks_the_precast_card_after_two_round_starts() {
    init_config();
    let fight = Fight {
        battle_id: Some(116385108),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                position: Some(1),
                team_type: Some(1),
                current_hp: Some(100),
                attr: Some(sonettobuf::HeroAttribute {
                    attack: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                position: Some(1),
                team_type: Some(2),
                current_hp: Some(100),
                attr: Some(sonettobuf::HeroAttribute {
                    attack: Some(200),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(1000),
                    buff_id: Some(116385679),
                    from_uid: Some(-1),
                    layer: Some(1),
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
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    assert!(
        managers
            .battle_rule
            .owned_skills()
            .any(|owned| owned == (crate::engine::fight::rules::ATTACKER_SIDE_UID, 116385684))
    );

    run_start(
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

    assert!(managers.buff.has_buff_id(10, 116385669));
    assert!(
        !managers
            .card
            .hand()
            .iter()
            .any(|card| { card.skill_id == Some(31446013) && card.temp_card == Some(true) })
    );

    managers.begin_round();
    run_round_start_split(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 2,
            ..Default::default()
        },
        1,
    )
    .unwrap();

    assert!(managers.buff.has_buff_id(10, 116385670));
    assert!(managers.card.hand().iter().any(|card| {
        card.skill_id == Some(31446013) && card.uid == Some(10) && card.temp_card == Some(true)
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
    assert_eq!(managers.ex_point.get(-1), 0);
}

#[test]
fn opening_round_start_late_uses_owner_eligibility_but_recurring_setup_stays_unfiltered() {
    init_config();
    fn fight() -> Fight {
        let entity = |uid, team_type| FightEntityInfo {
            uid: Some(uid),
            team_type: Some(team_type),
            current_hp: Some(100),
            passive_skill: vec![40],
            ..Default::default()
        };
        Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![entity(10, 1)],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![entity(-1, 2)],
                ..Default::default()
            }),
            ..Default::default()
        }
    }
    fn catalog() -> SkillEffectCatalog {
        let mut slot = SkillEffectSlot::new(
            ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
            TargetRequest::self_only(),
        );
        slot.conditions = vec![ParsedCondition {
            opcode: 2104,
            type_name: "LifeMore".to_owned(),
            kind: ParsedConditionKind::HpPermille {
                compare: crate::engine::skill::condition::ConditionCompare::GreaterThan,
                threshold: 500,
            },
            raw_args: vec!["500".to_owned()],
        }];
        slot.compiled_route = ConditionRoute::compile(&slot.conditions);
        let mut catalog = SkillEffectCatalog::default();
        catalog.insert(ParsedSkillEffect {
            skill_id: 40,
            slots: vec![slot],
        });
        catalog
    }

    let opening_fight = fight();
    let opening_pool = TargetPool::from_fight(&opening_fight);
    let mut opening_managers = BattleManagers::seeded(&opening_fight);
    run_start(
        opening_managers.catalog(),
        &mut opening_managers,
        &opening_pool,
        &catalog(),
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
    assert_eq!(opening_managers.ex_point.get(10), 1);
    assert_eq!(opening_managers.ex_point.get(-1), 0);

    let recurring_fight = fight();
    let recurring_pool = TargetPool::from_fight(&recurring_fight);
    let mut recurring_managers = BattleManagers::seeded(&recurring_fight);
    crate::engine::runtime::drain::run_setup_stage(
        &mut recurring_managers,
        &recurring_pool,
        &catalog(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        SetupStage::RoundStartLate,
        0,
    )
    .unwrap();
    assert_eq!(recurring_managers.ex_point.get(10), 1);
    assert_eq!(recurring_managers.ex_point.get(-1), 1);
}

#[test]
fn configured_round_after_runs_for_defenders_during_opening() {
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
            entitys: [-1, -2, -3]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    team_type: Some(2),
                    current_hp: Some(100),
                    passive_skill: vec![116362110],
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let (result, _) = run_start(
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

    for uid in [-1, -2, -3] {
        let buffs = managers
            .buff
            .active_for(uid)
            .filter(|buff| buff.buff_id == Some(116362001))
            .collect::<Vec<_>>();
        assert_eq!(buffs.len(), 1, "uid={uid}");
        assert_eq!(buffs[0].from_uid, Some(uid));
        assert_eq!(buffs[0].duration, Some(4));
        assert!(
            [116362002, 116362003, 116362004]
                .into_iter()
                .all(|buff_id| !managers.buff.has_buff_id(uid, buff_id))
        );
    }

    fn collect_round_after(step: &sonettobuf::FightStep, actions: &mut Vec<(i64, i64)>) -> bool {
        let mut found = false;
        if step.act_id == Some(116362110) {
            actions.push((
                step.from_id.expect("configured action has a source"),
                step.to_id.expect("configured action has a target"),
            ));
            found = true;
        }
        for child in step
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
        {
            found |= collect_round_after(child, actions);
        }
        found
    }

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let mut actions = Vec::new();
    let action_positions = steps
        .iter()
        .enumerate()
        .filter_map(|(index, step)| collect_round_after(step, &mut actions).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(actions, vec![(-1, -1), (-2, -2), (-3, -3)]);
    let deal_position = steps
        .iter()
        .position(|step| {
            step.act_effect.iter().any(|effect| {
                effect.effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Enterfightdeal as i32)
            })
        })
        .expect("opening projects card setup after round-start conditions");
    assert!(
        action_positions
            .into_iter()
            .all(|position| position < deal_position)
    );
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
fn version_seven_opening_reacts_before_late_duration_and_keeps_new_outputs_unsnapped() {
    init_config();
    let entity = |uid, team_type, passive_skill| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
        team_type: Some(team_type),
        passive_skill,
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, vec![40]), entity(11, 1, Vec::new())],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, Vec::new())],
            ..Default::default()
        }),
        ..Default::default()
    };
    let enter_fight_slot = |behavior| {
        let mut slot = SkillEffectSlot::new(behavior, TargetRequest::self_only());
        slot.conditions = vec![ParsedCondition {
            opcode: 5,
            type_name: "EnterFight".to_owned(),
            kind: ParsedConditionKind::Lifecycle(
                crate::engine::skill::condition::lifecycle::LifecycleMode::EnterFight,
            ),
            raw_args: Vec::new(),
        }];
        slot.compiled_route = ConditionRoute::compile(&slot.conditions);
        slot
    };
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 40,
        slots: vec![
            enter_fight_slot(ParsedBehavior::new(1, "AddBuff", vec![31070121])),
            enter_fight_slot(ParsedBehavior::new(
                60204,
                "AddBuffSpecialCount",
                vec![3, 31070121],
            )),
        ],
    });
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);

    let (opening, _) = run_start(
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

    let channel_reaction = opening
        .outcomes
        .iter()
        .position(|outcome| {
            matches!(
                outcome,
                RuleOutcome::BuffFeatureMarker(marker)
                    if marker.target_uid == 10
                        && marker.effect_type
                            == sonettobuf::effect_type_enum::EffectType::Triggeranalysis as i32
            )
        })
        .expect("opening round reacts to the configured Ulrich channel");
    let channel_advance = opening
        .outcomes
        .iter()
        .position(|outcome| {
            matches!(
                outcome,
                RuleOutcome::BuffBatch(changes)
                    if changes.iter().any(|change| {
                        change.origin.key.opcode
                            == crate::engine::skill::buff_act::effect_time::ROUND_START_AFTER_REACTION_DURATION
                            && change.change.refreshed.iter().any(|refresh| {
                                refresh.after.buff_id == Some(31070121)
                                    && refresh.after.duration == Some(2)
                            })
                    })
            )
        })
        .expect("opening round advances the configured takeStage 104 channel");
    assert!(channel_reaction < channel_advance);
    assert_eq!(
        managers
            .buff
            .active_for(10)
            .find(|buff| buff.buff_id == Some(31070121))
            .and_then(|buff| buff.duration),
        Some(2)
    );
    for owner_uid in [10, 11] {
        let output = managers
            .buff
            .active_for(owner_uid)
            .find(|buff| buff.buff_id == Some(31070151))
            .expect("opening channel grants its configured ally output");
        assert_eq!(output.duration, Some(1));
    }
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
        managers.catalog(),
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
