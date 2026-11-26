use super::*;

#[test]
fn round_start_refill_draws_before_round_start_cards_are_generated() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3124),
                current_hp: Some(100),
                ex_point: Some(10),
                ex_point_type: Some(3),
                skill_group1: vec![100],
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31242140),
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
    managers
        .execute_buff(
            crate::engine::manager::buff::BuffCommand::AccumulateActValue(
                crate::engine::manager::buff::BuffAccumulateActValue {
                    origin: CommandOrigin {
                        domain: RuleDomain::BuffAct,
                        key: DefinitionKey::new(10001, "AdrenalineAddCard"),
                    },
                    target_uid: 10,
                    buff_uid: 20,
                    act_id: 10001,
                    delta: 2,
                },
            ),
        )
        .unwrap();
    let card = CardInfo {
        uid: Some(10),
        skill_id: Some(100),
        ..Default::default()
    };
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![card.clone()],
            draw_pile: vec![card.clone()],
            deck_num: 1,
        }))
        .unwrap();
    let mut determinism = RoundDeterminism::default();

    let (_, next_round, _, _) = run_round_start_after_ai_split(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut determinism,
        TargetContext {
            current_round: 2,
            ..Default::default()
        },
        &[],
        2,
    )
    .unwrap();

    let cards = next_round
        .frames
        .iter()
        .flat_map(|frame| &frame.items)
        .find_map(|item| match item {
            FrameItem::Cue(RoundCue::NextRoundCards { cards, .. }) => Some(cards),
            FrameItem::Change(_) | FrameItem::Child(_) | FrameItem::Cue(_) => None,
        })
        .expect("round start emits the next hand");
    assert_eq!(
        cards
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![100, 100, 31242103]
    );
}

#[test]
fn opening_refill_defers_an_ultimate_made_ready_during_setup() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(4),
                ex_skill: Some(900),
                skill_group1: vec![100],
                passive_skill: vec![40],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut catalog = SkillEffectCatalog::default();
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
            hand: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(100),
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        1,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 5);
    assert_eq!(
        managers
            .card
            .hand()
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![100]
    );

    managers
        .execute_card(CardCommand::Play(CardPlay {
            origin: CARD_PLAY_ORIGIN,
            hand_index: 0,
            target_uid: None,
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }))
        .unwrap();
    run_round_start_refill(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 2,
            ..Default::default()
        },
        1,
        1,
    )
    .unwrap();

    assert_eq!(
        managers
            .card
            .hand()
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![900]
    );
}

#[test]
fn opening_refill_includes_an_ultimate_made_ready_during_enter_fight() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(4),
                ex_skill: Some(900),
                skill_group1: vec![100],
                passive_skill: vec![40],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut catalog = SkillEffectCatalog::default();
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
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
            hand: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(100),
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        1,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 5);
    assert_eq!(
        managers
            .card
            .hand()
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![900]
    );
}

#[test]
fn opening_keeps_the_exact_battle_start_ultimate_after_later_resource_loss() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(0),
                ex_skill: Some(900),
                skill_group1: vec![100],
                passive_skill: vec![40],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut catalog = SkillEffectCatalog::default();
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![5], Vec::new()),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 5021,
        type_name: "EnterFight".to_owned(),
        kind: ParsedConditionKind::Lifecycle(
            crate::engine::skill::condition::lifecycle::LifecycleMode::BattleStart,
        ),
        raw_args: Vec::new(),
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    let mut later_slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![-5], Vec::new()),
        TargetRequest::self_only(),
    );
    later_slot.conditions = vec![ParsedCondition {
        opcode: 5,
        type_name: "EnterFight".to_owned(),
        kind: ParsedConditionKind::Lifecycle(
            crate::engine::skill::condition::lifecycle::LifecycleMode::EnterFight,
        ),
        raw_args: Vec::new(),
    }];
    later_slot.compiled_route = ConditionRoute::compile(&later_slot.conditions);
    catalog.insert(ParsedSkillEffect {
        skill_id: 40,
        slots: vec![slot, later_slot],
    });

    run_start(
        managers.catalog(),
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        CardSetup {
            hand: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(100),
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        1,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 0);
    assert_eq!(
        managers
            .card
            .hand()
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![900]
    );
}

#[test]
fn opening_team_cards_use_battle_start_ultimate_readiness() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3139),
                current_hp: Some(100),
                ex_point: Some(0),
                ex_skill: Some(31390131),
                skill_group1: vec![100],
                passive_skill: vec![40],
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31390181),
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
    let mut catalog = SkillEffectCatalog::default();
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![5], Vec::new()),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 5021,
        type_name: "EnterFight".to_owned(),
        kind: ParsedConditionKind::Lifecycle(
            crate::engine::skill::condition::lifecycle::LifecycleMode::BattleStart,
        ),
        raw_args: Vec::new(),
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
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
            hand: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(100),
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        1,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 5);
    assert_eq!(
        managers
            .card
            .team_cards()
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![31390131]
    );
}

#[test]
fn opening_deal_keeps_entity_exempt_ultimate_ahead_of_ordinary_cards() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3157),
                team_type: Some(1),
                current_hp: Some(100),
                ex_skill: Some(31571131),
                passive_skill: vec![31571143],
                skill_group1: vec![31570111, 31570112],
                skill_group2: vec![31571121],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let card = |skill_id| CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        ..Default::default()
    };
    run_start(
        managers.catalog(),
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        CardSetup {
            hand: vec![card(31570111), card(31570111)],
            draw_pile: vec![card(31571121)],
            deck_num: 1,
        },
        2,
    )
    .unwrap();

    assert_eq!(
        managers
            .card
            .hand()
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![31571131, 31570112, 31571121]
    );
    assert_eq!(
        managers.card.opening_deal().unwrap()[0].skill_id,
        Some(31571131)
    );
}

#[test]
fn opening_setup_rejects_an_ineligible_seeded_ultimate() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(0),
                ex_skill: Some(900),
                skill_group1: vec![100],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);

    run_start(
        managers.catalog(),
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        CardSetup {
            hand: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(900),
                ..Default::default()
            }],
            draw_pile: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(100),
                ..Default::default()
            }],
            deck_num: 1,
        },
        1,
    )
    .unwrap();

    assert_eq!(
        managers
            .card
            .hand()
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![100]
    );
}

#[test]
fn configured_ultimate_alias_does_not_consume_the_incantation_deck() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3134),
                current_hp: Some(100),
                ex_point: Some(5),
                ex_skill: Some(31345131),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let ultimate_alias = CardInfo {
        uid: Some(10),
        skill_id: Some(31340131),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.card = crate::engine::manager::card::CardManager::with_draw_pile(
        Vec::new(),
        vec![ultimate_alias.clone()],
    );
    managers.card.set_deck_num(1);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_card_draws(vec![ultimate_alias]);

    run_round_refill(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut determinism,
        TargetContext::default(),
        1,
        1,
    )
    .unwrap();

    assert_eq!(managers.card.deck_num(), 1);
    assert_eq!(managers.card.hand()[0].skill_id, Some(31340131));
}

#[test]
fn round_refill_rejects_a_stale_ultimate_after_moxie_is_spent() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(1000),
                current_hp: Some(100),
                ex_point: Some(4),
                ex_skill: Some(900),
                skill_group1: vec![100],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let basic = CardInfo {
        uid: Some(10),
        skill_id: Some(100),
        ..Default::default()
    };
    let ultimate = CardInfo {
        uid: Some(10),
        skill_id: Some(900),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.card = crate::engine::manager::card::CardManager::with_draw_pile(
        vec![basic.clone()],
        vec![ultimate.clone(), basic.clone()],
    );
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_card_draws(vec![ultimate, basic]);

    run_round_refill(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut determinism,
        TargetContext::default(),
        2,
        1,
    )
    .unwrap();

    assert_eq!(managers.card.normal_hand_len(), 2);
    assert!(
        managers
            .card
            .hand()
            .iter()
            .all(|card| card.skill_id != Some(900))
    );
}

#[test]
fn round_refill_keeps_card_not_cal_size_ultimate_in_hand_but_outside_capacity() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3139),
                current_hp: Some(100),
                ex_point: Some(5),
                ex_skill: Some(31390131),
                skill_group1: vec![100],
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31390181),
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
    let normal = CardInfo {
        uid: Some(10),
        skill_id: Some(100),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.card = crate::engine::manager::card::CardManager::with_draw_pile(
        vec![normal.clone()],
        vec![normal],
    );

    run_round_refill(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        2,
        1,
    )
    .unwrap();

    assert_eq!(
        crate::engine::mechanic::card::CardMechanic.refill_hand_len(&managers, &pool),
        2
    );
    assert_eq!(
        managers
            .card
            .hand()
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![100, 31390131, 100]
    );
    assert!(managers.card.draw_pile().is_empty());
    run_round_start_refill(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        2,
        1,
    )
    .unwrap();

    assert_eq!(
        crate::engine::mechanic::card::CardMechanic.refill_hand_len(&managers, &pool),
        2
    );
    assert_eq!(managers.card.normal_hand_len(), 3);
    assert!(
        managers
            .card
            .hand()
            .iter()
            .any(|card| card.skill_id == Some(31390131))
    );
    let special = crate::engine::mechanic::card::CardMechanic.special_team_cards(
        &pool,
        &managers,
        managers.card.hand(),
    );
    assert!(special.is_empty());
}

#[test]
fn round_refill_replaces_played_normal_cards_even_when_precast_cards_fill_the_visible_hand() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let card = |skill_id| CardInfo {
        uid: Some(10),
        skill_id: Some(skill_id),
        temp_card: Some(false),
        ..Default::default()
    };
    let mut hand = (100..109).map(card).collect::<Vec<_>>();
    hand.extend(
        (800..803).map(|skill_id| crate::engine::manager::card::precast_card(10, skill_id)),
    );
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand,
            draw_pile: (200..203).map(card).collect(),
            deck_num: 3,
        }))
        .unwrap();
    for _ in 0..3 {
        managers
            .execute_card(CardCommand::Play(CardPlay {
                origin: CARD_PLAY_ORIGIN,
                hand_index: 0,
                target_uid: None,
                chosen_skill_id: None,
                choice: None,
                recorded_skill: None,
            }))
            .unwrap();
    }

    assert_eq!(managers.card.hand().len(), 9);
    assert_eq!(managers.card.normal_hand_len(), 6);

    run_round_refill(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        9,
        1,
    )
    .unwrap();

    assert_eq!(managers.card.normal_hand_len(), 9);
    assert_eq!(managers.card.hand().len(), 12);
    assert_eq!(managers.card.refilled().len(), 3);
    assert_eq!(
        managers
            .card
            .hand()
            .iter()
            .filter(|card| card.temp_card.unwrap_or_default())
            .count(),
        3
    );
}
