use super::*;

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
fn blocked_ai_card_still_gains_card_play_moxie() {
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
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(116_370_081),
                level: Some(175),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(4011),
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
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [117_373_053], []);

    let result = run_ai_actions(
        &fight,
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [AiSkillChoice {
            source_uid: -1,
            skill_id: 117_373_053,
            target_uid: 10,
        }],
        &[],
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(-1), 1);
    assert!(!result.events.iter().any(|event| matches!(
        event,
        crate::engine::event::payload::BattleEvent::SkillAction(action)
            if action.source_uid == -1
    )));
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
fn source_opening_effects_precede_the_ultimate_cost() {
    init_config();
    const SKILL_ID: i32 = 31_270_132;
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3127),
                team_type: Some(1),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    attack: Some(1_000),
                    hp: Some(10_000),
                    ..Default::default()
                }),
                ex_point: Some(5),
                ex_skill: Some(SKILL_ID),
                buffs: vec![BuffInfo {
                    uid: Some(40),
                    buff_id: Some(4_150_002),
                    count: Some(1),
                    layer: Some(6),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
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
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(SKILL_ID),
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 16,
        }))
        .unwrap();
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
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
    let skill = steps
        .iter()
        .find(|step| step.act_id == Some(SKILL_ID))
        .unwrap();
    let opening = skill
        .act_effect
        .iter()
        .position(|effect| effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(31_270_101))
        .expect("source-opening buff effect");
    let preignition = skill
        .act_effect
        .iter()
        .position(|effect| effect.buff.as_ref().and_then(|buff| buff.buff_id) == Some(4_150_001))
        .expect("preignition burn effect");
    let cost = skill
        .act_effect
        .iter()
        .position(|effect| {
            effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32)
                && effect.effect_num == Some(-5)
        })
        .expect("ultimate cost effect");
    let damage = skill
        .act_effect
        .iter()
        .position(|effect| {
            matches!(
                effect.effect_type,
                Some(kind)
                    if kind == sonettobuf::effect_type_enum::EffectType::Damage as i32
                        || kind == sonettobuf::effect_type_enum::EffectType::Crit as i32
            )
        })
        .expect("configured damage effect");
    assert!(opening < preignition && preignition < cost && cost < damage);
    assert_eq!(managers.ex_point.get(10), 0);
    assert!(!managers.buff.has_buff_id(10, 31_270_101));
    assert_eq!(managers.buff.buff_id_amount(-1, 4_150_001), 10);
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
