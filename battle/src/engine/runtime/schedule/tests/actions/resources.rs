use super::*;
use crate::engine::skill::action::{SkillExecutionMode, SkillLifecycle};

#[test]
fn full_named_boss_power_authorizes_an_ultimate_without_moxie() {
    let build_fight = |power| Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ex_point: Some(0),
                ex_skill: Some(900),
                power_infos: vec![PowerInfo {
                    power_id: Some(
                        crate::engine::manager::eureka::PowerType::ZongMaoBossEnergy.id(),
                    ),
                    num: Some(power),
                    max: Some(3),
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = build_fight(3);
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 900,
        slots: Vec::new(),
    });

    run_active_action(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        ActiveActionRequest {
            skill: crate::engine::skill::action::SkillRequest {
                source_uid: -1,
                skill_id: 900,
            }
            .into(),
            grants_ex_point: true,
            grant_after_action: false,
            queued_resource_delta: 0,
            prelude: Vec::new(),
        },
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(-1), 0);

    let underfilled = build_fight(2);
    let error = match run_active_action(
        &mut BattleManagers::seeded(&underfilled),
        &TargetPool::from_fight(&underfilled),
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        ActiveActionRequest {
            skill: crate::engine::skill::action::SkillRequest {
                source_uid: -1,
                skill_id: 900,
            }
            .into(),
            grants_ex_point: true,
            grant_after_action: false,
            queued_resource_delta: 0,
            prelude: Vec::new(),
        },
    ) {
        Ok(_) => panic!("underfilled boss power accepted an ultimate"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        DrainError::InsufficientUltimateResource {
            owner_uid: -1,
            skill_id: 900,
            required: 3,
            current: 2,
        }
    );
}

#[test]
fn player_owned_negative_uid_gains_card_play_moxie() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(1),
                current_hp: Some(100),
                ex_point: Some(0),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![CardInfo {
                uid: Some(-1),
                skill_id: Some(100),
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 1,
        }))
        .unwrap();
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: Vec::new(),
    });

    run_player_action_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [CardPlay {
            origin: CARD_PLAY_ORIGIN,
            hand_index: 0,
            target_uid: None,
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }],
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(-1), 1);
}

#[test]
fn precast_card_does_not_grant_card_play_moxie() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                ex_point: Some(0),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![crate::engine::manager::card::precast_card(10, 100)],
            draw_pile: Vec::new(),
            deck_num: 1,
        }))
        .unwrap();
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: Vec::new(),
    });

    run_player_action_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [CardPlay {
            origin: CARD_PLAY_ORIGIN,
            hand_index: 0,
            target_uid: None,
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }],
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 0);
}

#[test]
fn ultimate_spend_is_recorded_inside_its_skill_frame() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(5),
                ex_skill: Some(999),
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(5081),
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
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(999),
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 16,
        }))
        .unwrap();
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 999,
        slots: Vec::new(),
    });

    let result = run_player_action_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [CardPlay {
            origin: CARD_PLAY_ORIGIN,
            hand_index: 0,
            target_uid: None,
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }],
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 0);
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let skill = steps.iter().find(|step| step.act_id == Some(999)).unwrap();
    assert!(
        skill.act_effect.iter().any(|effect| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32)
                && effect.effect_num == Some(-5)
        }),
        "{steps:#?}"
    );
}

#[test]
fn configured_ultimate_alias_spends_moxie() {
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
    let mut managers = BattleManagers::seeded(&fight);
    let mut hand = (100..106)
        .map(|skill_id| CardInfo {
            uid: Some(10),
            skill_id: Some(skill_id),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    hand.push(crate::engine::manager::card::precast_card(10, 31345153));
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand,
            draw_pile: Vec::new(),
            deck_num: 16,
        }))
        .unwrap();
    managers
        .execute_card(CardCommand::SetTeamCards(
            crate::engine::manager::card::CardSetTeamCards {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "TestTeamCards"),
                },
                cards: vec![CardInfo {
                    uid: Some(10),
                    skill_id: Some(31340131),
                    ..Default::default()
                }],
            },
        ))
        .unwrap();
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 31340131,
        slots: Vec::new(),
    });

    run_player_action_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [CardPlay {
            origin: CARD_PLAY_ORIGIN,
            hand_index: 7,
            target_uid: None,
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }],
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 0);
}

#[test]
fn lorentz_rewritten_precast_does_not_grant_moxie() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(0),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(100),
                temp_card: Some(true),
                enchants: vec![sonettobuf::CardEnchant {
                    enchant_id: Some(10_010),
                    duration: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: Vec::new(),
    });

    run_player_action_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [CardPlay {
            origin: CARD_PLAY_ORIGIN,
            hand_index: 0,
            target_uid: None,
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }],
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 0);
}

#[test]
fn action_start_subscribers_settle_before_the_ultimate_cost_frame() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(5),
                ex_skill: Some(999),
                passive_skill: vec![1_000],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(999),
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 16,
        }))
        .unwrap();
    let mut subscriber = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
        TargetRequest::self_only(),
    );
    subscriber.conditions = vec![ParsedCondition {
        opcode: 201,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(NoneMode::SkillActionStart),
        raw_args: Vec::new(),
    }];
    subscriber.compiled_route = ConditionRoute::compile(&subscriber.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 1_000,
        slots: vec![subscriber],
    });
    catalog.insert(ParsedSkillEffect {
        skill_id: 999,
        slots: Vec::new(),
    });

    let result = run_player_action_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [CardPlay {
            origin: CARD_PLAY_ORIGIN,
            hand_index: 0,
            target_uid: None,
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }],
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let skill = steps.iter().find(|step| step.act_id == Some(999)).unwrap();
    let effect_types = skill
        .act_effect
        .iter()
        .filter_map(|effect| effect.effect_type)
        .collect::<Vec<_>>();
    assert_eq!(
        effect_types,
        vec![
            sonettobuf::effect_type_enum::EffectType::Fightstep as i32,
            sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
        ]
    );
    assert_eq!(managers.ex_point.get(10), 1);
}

#[test]
fn ultimate_spends_the_required_cost_derived_from_its_active_buff() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(222001),
                current_hp: Some(100),
                ex_point: Some(1),
                ex_skill: Some(222001231),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(2220012),
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
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(222001231),
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 16,
        }))
        .unwrap();
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 222001231,
        slots: Vec::new(),
    });

    run_player_action_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [CardPlay {
            origin: CARD_PLAY_ORIGIN,
            hand_index: 0,
            target_uid: None,
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }],
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 0);
}

#[test]
fn conduit_attack_does_not_begin_without_a_living_enemy() {
    init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(100),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut twins = entity(10, 3149);
    twins.ex_skill = Some(31490151);
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![twins],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1001)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.hp.lose(-1, 100, 0);
    managers
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::SelectGroup {
                source_uid: 10,
                group: 3,
            },
        )
        .unwrap();
    let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let result = run_conduit_phase(
        managers.catalog(),
        &fight,
        &mut managers,
        &pool,
        &mut catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[sonettobuf::FightDeviceOper {
            uid: Some(10),
            index: Some(3),
        }],
    )
    .unwrap();

    assert!(result.frames.is_empty());
    assert_eq!(managers.conduit.uses(10), 0);
}

#[test]
fn conduit_source_target_uses_the_first_living_main_ally_as_its_frame_anchor() {
    init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(20, 3134), entity(10, 3149)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1001)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let result = run_conduit_phase(
        managers.catalog(),
        &fight,
        &mut managers,
        &pool,
        &mut catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[sonettobuf::FightDeviceOper {
            uid: Some(10),
            index: Some(1),
        }],
    )
    .unwrap();

    assert!(matches!(
        result.frames.first().map(|frame| &frame.owner),
        Some(crate::engine::runtime::record::FrameOwner::ConduitAction {
            target_uid: Some(20),
            ..
        })
    ));
    assert!(result.frames[0].items.iter().any(|item| matches!(
        item,
        crate::engine::runtime::record::FrameItem::Child(frame)
            if matches!(
                frame.owner,
                crate::engine::runtime::record::FrameOwner::ConduitSkill {
                    source_uid: 10,
                    skill_id: 31490111,
                    card_index: 1,
                    target_uid: Some(20),
                    ..
                }
            )
    )));
    assert_eq!(managers.conduit.power(1, 1), 1);
}

#[test]
fn device_power_card_keeps_ally_action_without_granting_loop_chain() {
    init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, 3149),
                FightEntityInfo {
                    passive_skill: vec![31430151],
                    skill_group1: vec![31430111],
                    ..entity(30, 3143)
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1001)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![
                CardInfo {
                    uid: Some(10),
                    skill_id: Some(31446011),
                    temp_card: Some(false),
                    card_type: Some(0),
                    hero_id: Some(3149),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(30),
                    skill_id: Some(31430111),
                    hero_id: Some(3143),
                    ..Default::default()
                },
            ],
            draw_pile: Vec::new(),
            deck_num: 2,
        }))
        .unwrap();
    let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    managers
        .catalog()
        .extend_skill_roots(&mut catalog, [31446011], std::iter::empty());
    let play = || CardPlay {
        origin: CARD_PLAY_ORIGIN,
        hand_index: 0,
        target_uid: None,
        chosen_skill_id: None,
        choice: None,
        recorded_skill: None,
    };

    let device = run_player_action_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [play()],
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();

    assert!(device.outcomes.iter().any(|outcome| matches!(
        outcome,
        RuleOutcome::SkillLifecycle(SkillLifecycle::ActionCompleted(action))
            if action.skill_id == 31446011 && action.mode == SkillExecutionMode::DeviceCard
    )));
    assert!(device.events.iter().any(|event| matches!(
        event,
        BattleEvent::AllyAction(action)
            if action.skill_id == 31446011 && action.mode == SkillExecutionMode::DeviceCard
    )));
    assert_eq!(managers.buff.buff_id_amount(30, 31430151), 0);

    let active = run_player_action_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [play()],
        1,
        crate::engine::manager::emitter::UID,
    )
    .unwrap();

    assert!(active.events.iter().any(|event| matches!(
        event,
        BattleEvent::AllyAction(action)
            if action.skill_id == 31430111 && action.mode == SkillExecutionMode::Active
    )));
    assert_eq!(managers.buff.buff_id_amount(30, 31430151), 1);

    fn find_step(step: &sonettobuf::FightStep, act_id: i32) -> Option<&sonettobuf::FightStep> {
        (step.act_id == Some(act_id)).then_some(step).or_else(|| {
            step.act_effect
                .iter()
                .filter_map(|effect| effect.fight_step.as_ref())
                .find_map(|nested| find_step(nested, act_id))
        })
    }

    let steps = crate::engine::packet::timeline::project(&active.frames).unwrap();
    let reaction = steps
        .iter()
        .find_map(|step| find_step(step, 31430151))
        .unwrap();
    assert_eq!(reaction.to_id, Some(-1));
    assert!(reaction.act_effect.iter().any(|effect| {
        effect.target_id == Some(30)
            && effect
                .buff
                .as_ref()
                .is_some_and(|buff| buff.buff_id == Some(31430151))
    }));
}

#[test]
fn conduit_attacks_complete_loop_chain_and_grant_moxie() {
    init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, 3149),
                FightEntityInfo {
                    ex_point: Some(4),
                    ex_skill: Some(31430131),
                    passive_skill: vec![31430141, 31430151],
                    ..entity(30, 3143)
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1001)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };
    managers
        .execute_buff(crate::engine::manager::buff::BuffCommand::Grant(
            crate::engine::manager::buff::BuffGrant {
                origin,
                source_uid: 30,
                target_uid: 30,
                buff_id: 31430151,
                amount: Some(3),
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))
        .unwrap();
    managers
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::ChangePower(
                crate::engine::manager::conduit::ConduitPowerChange {
                    origin,
                    source_uid: 10,
                    team: 1,
                    power_id: 1,
                    delta: 3,
                    kind: crate::engine::manager::conduit::ConduitPowerChangeKind::Standard,
                },
            ),
        )
        .unwrap();
    let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let result = run_conduit_phase(
        managers.catalog(),
        &fight,
        &mut managers,
        &pool,
        &mut catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[sonettobuf::FightDeviceOper {
            uid: Some(10),
            index: Some(1),
        }],
    )
    .unwrap();

    for skill_id in [31490111, 31490121] {
        assert!(result.events.iter().any(|event| matches!(
            event,
            BattleEvent::AllyAction(action)
                if action.skill_id == skill_id && action.mode == SkillExecutionMode::Device
        )));
    }
    assert_eq!(managers.buff.buff_id_amount(30, 31430151), 1);
    assert_eq!(managers.ex_point.get(30), 5);

    fn find_step(step: &sonettobuf::FightStep, act_id: i32) -> Option<&sonettobuf::FightStep> {
        (step.act_id == Some(act_id)).then_some(step).or_else(|| {
            step.act_effect
                .iter()
                .filter_map(|effect| effect.fight_step.as_ref())
                .find_map(|nested| find_step(nested, act_id))
        })
    }
    fn collect_loop_chain_layers(step: &sonettobuf::FightStep, layers: &mut Vec<i32>) {
        for effect in &step.act_effect {
            if let Some(buff) = effect
                .buff
                .as_ref()
                .filter(|buff| buff.buff_id == Some(31430151))
            {
                layers.push(buff.layer.unwrap_or_default());
            }
            if let Some(nested) = effect.fight_step.as_ref() {
                collect_loop_chain_layers(nested, layers);
            }
        }
    }

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let mut layers = Vec::new();
    for step in &steps {
        collect_loop_chain_layers(step, &mut layers);
    }
    assert!(layers.windows(2).any(|layers| layers == [3, 4]));
    assert!(layers.windows(2).any(|layers| layers == [4, 5]));
    let threshold = steps
        .iter()
        .find_map(|step| find_step(step, 31430141))
        .unwrap();
    assert!(threshold.act_effect.iter().any(|effect| {
        effect
            .fight_step
            .as_ref()
            .is_some_and(|step| step.act_id == Some(31430181))
    }));
}

#[test]
fn conduit_attacks_keep_their_lifecycle_without_becoming_active_incantations() {
    init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(20, 3134), entity(10, 3149)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1001)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_buff(crate::engine::manager::buff::BuffCommand::Grant(
            crate::engine::manager::buff::BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: 20,
                target_uid: 20,
                buff_id: 31340006,
                amount: Some(1),
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))
        .unwrap();
    assert!(managers.emanation.select(20, 10));
    let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let result = run_conduit_phase(
        managers.catalog(),
        &fight,
        &mut managers,
        &pool,
        &mut catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[sonettobuf::FightDeviceOper {
            uid: Some(10),
            index: Some(1),
        }],
    )
    .unwrap();
    assert!(result.outcomes.iter().any(|outcome| matches!(
        outcome,
        RuleOutcome::SkillLifecycle(SkillLifecycle::ActionCompleted(action))
            if action.mode == SkillExecutionMode::Device
    )));
    assert!(result.events.iter().any(|event| matches!(
        event,
        BattleEvent::SkillAction(action)
            if action.mode == SkillExecutionMode::Device
                && action.is_attack
    )));
    assert_eq!(managers.buff.buff_id_amount(-1, 31340001), 0);
}

#[test]
fn conduit_repeats_a_paid_skill_until_its_energy_is_spent() {
    init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 3149)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1001)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let origin = crate::engine::skill::rule::CommandOrigin {
        domain: crate::engine::skill::rule::RuleDomain::Behavior,
        key: crate::engine::skill::rule::DefinitionKey::new(60291, "AddDevicePower"),
    };
    managers
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::ChangePower(
                crate::engine::manager::conduit::ConduitPowerChange {
                    origin,
                    source_uid: 10,
                    team: 1,
                    power_id: 1,
                    delta: 2,
                    kind: crate::engine::manager::conduit::ConduitPowerChangeKind::Standard,
                },
            ),
        )
        .unwrap();
    let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    run_conduit_phase(
        managers.catalog(),
        &fight,
        &mut managers,
        &pool,
        &mut catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[sonettobuf::FightDeviceOper {
            uid: Some(10),
            index: Some(1),
        }],
    )
    .unwrap();

    assert_eq!(managers.conduit.power(1, 1), 0);
    assert_eq!(managers.conduit.consumed(1, 1), 6);
    assert_eq!(managers.conduit.uses(10), 3);
}

#[test]
fn conduit_does_not_start_another_device_after_battle_ends() {
    init_config();
    let entity = |uid, model_id, hp| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(hp),
        attr: Some(HeroAttribute {
            hp: Some(hp),
            attack: Some(10_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 3149, 100_000), entity(11, 3149, 100_000)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1001, 1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let origin = crate::engine::skill::rule::CommandOrigin {
        domain: crate::engine::skill::rule::RuleDomain::Behavior,
        key: crate::engine::skill::rule::DefinitionKey::new(60291, "AddDevicePower"),
    };
    managers
        .conduit
        .execute(
            crate::engine::manager::conduit::ConduitCommand::ChangePower(
                crate::engine::manager::conduit::ConduitPowerChange {
                    origin,
                    source_uid: 10,
                    team: 1,
                    power_id: 1,
                    delta: 2,
                    kind: crate::engine::manager::conduit::ConduitPowerChangeKind::Standard,
                },
            ),
        )
        .unwrap();
    let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    run_conduit_phase(
        managers.catalog(),
        &fight,
        &mut managers,
        &pool,
        &mut catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[
            sonettobuf::FightDeviceOper {
                uid: Some(10),
                index: Some(1),
            },
            sonettobuf::FightDeviceOper {
                uid: Some(11),
                index: Some(1),
            },
        ],
    )
    .unwrap();

    assert_eq!(managers.hp.current(-1), 0);
    assert_eq!(managers.conduit.uses(11), 0);
}
