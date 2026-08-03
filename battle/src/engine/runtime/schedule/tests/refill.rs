use super::*;

#[test]
fn ai_queue_refresh_awards_composition_moxie_from_configured_card_ranks() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(100),
                    ex_point: Some(5),
                    skill_group1: vec![100, 101],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-3),
                    current_hp: Some(100),
                    ex_point: Some(3),
                    skill_group1: vec![100, 101],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);

    let result = run_ai_queue_refresh(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        vec![
            CardInfo {
                uid: Some(-2),
                skill_id: Some(101),
                ..Default::default()
            },
            CardInfo {
                uid: Some(-3),
                skill_id: Some(101),
                ..Default::default()
            },
        ],
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(-2), 5);
    assert_eq!(managers.ex_point.get(-3), 4);
    assert_eq!(
        managers
            .card
            .ai_queue()
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![101, 101]
    );
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].act_effect[0].target_id, Some(-3));
    assert_eq!(
        steps[0].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32)
    );
    assert_eq!(steps[0].act_effect[0].effect_num, Some(1));
}

#[test]
fn round_refill_commits_draw_compose_moxie_and_deck_count_in_order() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(0),
                skill_group1: vec![100, 101],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let candidate = CardInfo {
        uid: Some(10),
        skill_id: Some(100),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(100),
                ..Default::default()
            }],
            draw_pile: vec![candidate.clone(), candidate.clone()],
            deck_num: 16,
        }))
        .unwrap();

    let mut result = run_round_deal(1);
    append(
        &mut result,
        run_round_refill(
            &mut managers,
            &pool,
            &SkillEffectCatalog::default(),
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            2,
            1,
        )
        .unwrap(),
    );

    assert_eq!(
        managers
            .card
            .hand()
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![101, 100]
    );
    assert_eq!(managers.card.deck_num(), 14);
    assert_eq!(
        managers
            .card
            .refilled()
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        vec![100, 100]
    );
    assert_eq!(managers.ex_point.get(10), 1);
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let effects = steps
        .iter()
        .flat_map(|step| &step.act_effect)
        .filter_map(|effect| effect.effect_type)
        .collect::<Vec<_>>();
    let ex_point = sonettobuf::effect_type_enum::EffectType::Expointchange as i32;

    let compose = sonettobuf::effect_type_enum::EffectType::Cardscompose as i32;
    let deck = sonettobuf::effect_type_enum::EffectType::Carddecknum as i32;
    assert!(
        effects.iter().position(|effect| *effect == ex_point)
            < effects.iter().position(|effect| *effect == compose)
    );
    assert_eq!(effects.last(), Some(&deck));
    assert_eq!(
        effects.iter().filter(|effect| **effect == compose).count(),
        1
    );
    assert!(steps.iter().any(|step| {
        step.act_effect
            .iter()
            .filter_map(|effect| effect.effect_type)
            .eq([ex_point, compose, deck])
    }));
}

#[test]
fn round_refill_recycles_an_exhausted_draw_pile_and_finishes_the_hand() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(1000),
                current_hp: Some(100),
                ex_point: Some(5),
                ex_skill: Some(900),
                skill_group1: vec![100],
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
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let precast = crate::engine::manager::card::precast_card(10, 800);
    let normal = card(100);
    let device = card(31490201);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![precast],
            draw_pile: vec![normal.clone(), device.clone()],
            deck_num: 1,
        }))
        .unwrap();
    let ultimate = card(900);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_card_draws(vec![normal, device, ultimate, card(100)]);

    let result = run_round_refill(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut determinism,
        TargetContext::default(),
        4,
        1,
    )
    .unwrap();

    assert_eq!(managers.card.normal_hand_len(), 4);
    assert_eq!(managers.card.hand().len(), 5);
    assert_eq!(managers.card.deck_num(), 0);
    assert_eq!(
        managers
            .card
            .hand()
            .iter()
            .filter(|card| card.temp_card.unwrap_or_default())
            .count(),
        1
    );
    assert_eq!(
        managers
            .card
            .hand()
            .iter()
            .filter(|card| card.skill_id == Some(900))
            .count(),
        1
    );
    let deck = sonettobuf::effect_type_enum::EffectType::Carddecknum as i32;
    assert_eq!(
        crate::engine::packet::timeline::project(&result.frames)
            .unwrap()
            .iter()
            .flat_map(|step| &step.act_effect)
            .filter(|effect| effect.effect_type == Some(deck))
            .filter_map(|effect| effect.effect_num)
            .collect::<Vec<_>>(),
        vec![1, 0]
    );
}

#[test]
fn round_refill_does_not_grant_composition_moxie_to_a_special_resource() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(0),
                ex_point_type: Some(
                    crate::engine::manager::ex_point::ExPointKind::Adrenaline.as_wire(),
                ),
                skill_group1: vec![100, 101],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let card = CardInfo {
        uid: Some(10),
        skill_id: Some(100),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![card.clone()],
            draw_pile: vec![card.clone(), card],
            deck_num: 16,
        }))
        .unwrap();

    let result = run_round_refill(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        2,
        1,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 0);
    let compose = sonettobuf::effect_type_enum::EffectType::Cardscompose as i32;
    assert!(
        crate::engine::packet::timeline::project(&result.frames)
            .unwrap()
            .iter()
            .flat_map(|step| &step.act_effect)
            .any(|effect| effect.effect_type == Some(compose))
    );
}

#[test]
fn round_refill_owns_the_final_deck_count_without_a_draw() {
    init_config();
    let fight = Fight::default();
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.card.set_deck_num(30);

    let result = run_round_refill(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        0,
        1,
    )
    .unwrap();

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert_eq!(steps.len(), 1);
    assert_eq!(
        steps[0].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Carddecknum as i32)
    );
    assert_eq!(steps[0].act_effect[0].effect_num, Some(30));
}

#[test]
fn round_refill_uses_one_normal_slot_for_a_unique_ultimate() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(1000),
                current_hp: Some(100),
                ex_point: Some(5),
                ex_skill: Some(900),
                skill_group1: vec![100],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let candidate = CardInfo {
        uid: Some(10),
        skill_id: Some(100),
        ..Default::default()
    };
    let precast = crate::engine::manager::card::precast_card(10, 800);

    let mut managers = BattleManagers::seeded(&fight);
    managers.card = crate::engine::manager::card::CardManager::with_draw_pile(
        vec![candidate.clone(), precast.clone()],
        vec![candidate.clone()],
    );
    managers.card.set_deck_num(16);
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
    assert_eq!(managers.card.hand().len(), 3);
    assert_eq!(managers.card.deck_num(), 16);
    assert_eq!(
        managers
            .card
            .hand()
            .iter()
            .filter(|card| card.skill_id == Some(900))
            .count(),
        1
    );

    let ultimate = CardInfo {
        uid: Some(10),
        skill_id: Some(900),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.card = crate::engine::manager::card::CardManager::with_draw_pile(
        vec![ultimate, precast.clone()],
        vec![candidate.clone(), candidate.clone()],
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
        managers
            .card
            .hand()
            .iter()
            .filter(|card| card.skill_id == Some(900))
            .count(),
        1
    );
    assert_eq!(
        crate::engine::mechanic::card::CardMechanic.refill_hand_len(&managers, &pool),
        2
    );

    let ultimate = CardInfo {
        uid: Some(10),
        skill_id: Some(900),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.card = crate::engine::manager::card::CardManager::with_draw_pile(
        vec![ultimate.clone(), precast],
        vec![candidate.clone(), candidate.clone()],
    );
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_card_draws(vec![ultimate, candidate]);
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

    assert_eq!(
        crate::engine::mechanic::card::CardMechanic.refill_hand_len(&managers, &pool),
        2
    );
    assert_eq!(
        managers
            .card
            .hand()
            .iter()
            .filter(|card| card.skill_id == Some(900))
            .count(),
        1
    );
}

#[test]
fn round_start_refill_adds_a_newly_ready_ultimate_to_a_full_hand() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(1000),
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
    managers.card = crate::engine::manager::card::CardManager::new(vec![CardInfo {
        uid: Some(10),
        skill_id: Some(100),
        ..Default::default()
    }]);
    let mut catalog = SkillEffectCatalog::default();
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 103,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(NoneMode::RoundStart),
        raw_args: Vec::new(),
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    catalog.insert(ParsedSkillEffect {
        skill_id: 40,
        slots: vec![slot],
    });
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_card_draws(vec![CardInfo {
        uid: Some(10),
        skill_id: Some(100),
        ..Default::default()
    }]);

    let (_, next_round, _, _) = run_round_start_after_ai_split(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext {
            current_round: 2,
            ..Default::default()
        },
        &[],
        1,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 5);
    assert!(determinism.has_queued_card_draw());
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
        vec![100, 900]
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
