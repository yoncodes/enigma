use super::*;

#[test]
fn action_queue_commit_records_cards_in_an_independent_buff_act_frame() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(33),
                    buff_id: Some(31130101),
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
                skill_id: Some(100),
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 1,
        }))
        .unwrap();
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

    let result = run_action_queue_committed(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        1,
        99_998,
    )
    .unwrap();
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();

    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].act_effect.len(), 3);
    let marker = &steps[1].act_effect[0];
    assert_eq!(
        marker.effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Fightstep as i32)
    );
    let nested = marker.fight_step.as_ref().unwrap();
    assert_eq!(nested.act_id, Some(31130101));
    assert_eq!(nested.from_id, Some(10));
    assert_eq!(nested.to_id, Some(10));
    assert_eq!(nested.act_effect[0].buff.as_ref().unwrap().uid, Some(33));
}

#[test]
fn action_queue_commit_expires_the_committed_teams_before_ap_buffs() {
    init_config();
    let entity = |uid, team_type, buff_uid| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(100),
        buffs: vec![BuffInfo {
            uid: Some(buff_uid),
            buff_id: Some(31390190),
            from_uid: Some(uid),
            duration: Some(1),
            ..Default::default()
        }],
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 31), entity(20, 1, 32)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, 33)],
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
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 1,
        }))
        .unwrap();
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

    let result = run_action_queue_committed(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        1,
        99_998,
    )
    .unwrap();

    assert!(managers.buff.snapshot(10, 31).is_none());
    assert!(managers.buff.snapshot(20, 32).is_none());
    assert!(managers.buff.snapshot(-1, 33).is_some());
    let removed = result
        .outcomes
        .iter()
        .find_map(|outcome| match outcome {
            RuleOutcome::Buff(changes) if changes.origin.key.opcode == 107 => Some(
                changes
                    .change
                    .removed
                    .iter()
                    .map(|removed| (removed.target_uid, removed.buff.uid))
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .unwrap();
    assert_eq!(removed, vec![(10, Some(31)), (20, Some(32))]);
}

#[test]
fn active_precast_card_publishes_moxie_after_its_card_indexed_skill() {
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
            hand: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(100),
                temp_card: Some(true),
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 30,
        }))
        .unwrap();
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
            TargetRequest::self_only(),
        )],
    });
    let result = run_player_action_queue(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [CardPlay {
            origin: crate::engine::manager::card::CARD_PLAY_ORIGIN,
            hand_index: 0,
            target_uid: None,
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }],
        1,
        0,
    )
    .unwrap();
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();

    assert_eq!(steps.len(), 3);
    assert_eq!(steps[0].act_effect.len(), 3);
    assert_eq!(steps[1].act_id, Some(100));
    assert_eq!(steps[1].card_index, Some(1));
    assert_eq!(steps[2].act_id, Some(0));
    assert_eq!(steps[2].act_effect[0].target_id, Some(10));
    assert_eq!(
        steps[2].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32)
    );
    assert_eq!(managers.ex_point.get(10), 2);
}

#[test]
fn dead_queued_target_retargets_the_next_living_enemy() {
    init_config();
    let card = CardInfo {
        uid: Some(10),
        skill_id: Some(100),
        ..Default::default()
    };
    let fight = Fight {
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
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    team_type: Some(2),
                    current_hp: Some(0),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    team_type: Some(2),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![card],
            draw_pile: Vec::new(),
            deck_num: 1,
        }))
        .unwrap();

    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
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
            target_uid: Some(-1),
            chosen_skill_id: None,
            choice: None,
            recorded_skill: None,
        }],
        1,
        0,
    )
    .unwrap();
    let effects = crate::engine::packet::timeline::project(&result.frames)
        .unwrap()
        .into_iter()
        .flat_map(|step| step.act_effect)
        .filter_map(|effect| effect.effect_type)
        .collect::<Vec<_>>();

    let target_uid = result.outcomes.iter().find_map(|outcome| {
        let RuleOutcome::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::ActionCompleted(action),
        ) = outcome
        else {
            return None;
        };
        (action.skill_id == 100).then_some(action.target_uid)
    });

    assert!(managers.card.hand().is_empty());
    assert_eq!(target_uid, Some(-2));
    assert!(!effects.contains(&(sonettobuf::effect_type_enum::EffectType::Cardinvalid as i32)));
    assert!(!effects.contains(&(sonettobuf::effect_type_enum::EffectType::Addhandcard as i32)));
}

#[test]
fn cards_without_a_living_current_enemy_resolve_support_effects_or_only_grant_moxie() {
    init_config();
    let run = |is_support| {
        let fight = Fight {
            battle_id: Some(2514),
            cur_wave: Some(1),
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
        managers
            .execute_hp(crate::engine::manager::hp::HpCommand::Kill(
                crate::engine::manager::hp::HpKill {
                    origin: CARD_PLAY_ORIGIN,
                    source_uid: 10,
                    target_uid: -1,
                    config_effect: 1,
                },
            ))
            .unwrap();
        managers
            .execute_card(CardCommand::Setup(CardSetup {
                hand: vec![CardInfo {
                    uid: Some(10),
                    skill_id: Some(100),
                    ..Default::default()
                }],
                draw_pile: Vec::new(),
                deck_num: 1,
            }))
            .unwrap();
        assert!(managers.wave.has_next_wave());
        assert!(!crate::engine::round::outcome::battle_ended(
            &fight, &pool, &managers
        ));
        let mut catalog = SkillEffectCatalog::default();
        catalog.insert(ParsedSkillEffect {
            skill_id: 100,
            slots: vec![SkillEffectSlot::new(
                ParsedBehavior::from_spec(
                    BehaviorSpec::new(20002, "AddExPoint"),
                    vec![1],
                    Vec::new(),
                ),
                TargetRequest::self_only(),
            )],
        });
        if is_support {
            catalog.insert_logic_target(
                100,
                crate::engine::skill::target::request::SOURCE_TARGET_CODE,
            );
        } else {
            catalog.insert_logic_target(100, 201);
            catalog.insert_damage_rate(100, 1_000);
        }
        let result = run_player_phase(
            &fight,
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            [RoundCommand::PlayCard {
                card_index: 0,
                target_uid: Some(-1),
                chosen_skill_id: None,
                recorded_skill: None,
            }],
            1,
            0,
        )
        .unwrap();
        (result, managers)
    };

    let (support, support_managers) = run(true);
    assert_eq!(support_managers.ex_point.get(10), 2);
    assert!(support.outcomes.iter().any(|outcome| matches!(
        outcome,
        RuleOutcome::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::ActionCompleted(action)
        ) if action.skill_id == 100
    )));

    let (attack, attack_managers) = run(false);
    assert_eq!(attack_managers.ex_point.get(10), 1);
    let steps = crate::engine::packet::timeline::project(&attack.frames).unwrap();
    let invalid = steps
        .iter()
        .find(|step| {
            step.act_effect.iter().any(|effect| {
                effect.effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Cardinvalid as i32)
            })
        })
        .unwrap();
    assert_eq!(invalid.from_id, Some(10));
    assert_eq!(invalid.act_effect[0].config_effect, Some(-1));
    let invalid_index = steps.iter().position(|step| step == invalid).unwrap();
    let reward_index = steps
        .iter()
        .position(|step| {
            step.act_effect.iter().any(|effect| {
                effect.effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32)
            })
        })
        .unwrap();
    assert!(invalid_index < reward_index);
}

#[test]
fn terminal_player_action_stops_the_remaining_action_queue() {
    init_config();
    let entity = |uid, hp| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(hp),
        attr: Some(HeroAttribute {
            hp: Some(hp),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1_000), entity(20, 1_000)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 1)],
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
                    skill_id: Some(100),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(20),
                    skill_id: Some(200),
                    ..Default::default()
                },
            ],
            draw_pile: Vec::new(),
            deck_num: 2,
        }))
        .unwrap();
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: Vec::new(),
    });
    catalog.insert_logic_target(100, 201);
    catalog.insert_damage_rate(100, 1_000);
    catalog.insert(ParsedSkillEffect {
        skill_id: 200,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
            TargetRequest::self_only(),
        )],
    });
    catalog.insert_logic_target(
        200,
        crate::engine::skill::target::request::SOURCE_TARGET_CODE,
    );

    let result = run_player_phase(
        &fight,
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [
            RoundCommand::PlayCard {
                card_index: 0,
                target_uid: Some(-1),
                chosen_skill_id: None,
                recorded_skill: None,
            },
            RoundCommand::PlayCard {
                card_index: 0,
                target_uid: None,
                chosen_skill_id: None,
                recorded_skill: None,
            },
        ],
        1,
        0,
    )
    .unwrap();

    assert_eq!(managers.hp.current(-1), 0);
    assert_eq!(managers.ex_point.get(20), 0);
    assert!(!result.outcomes.iter().any(|outcome| matches!(
        outcome,
        RuleOutcome::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::ActionCompleted(action)
        ) if action.skill_id == 200
    )));
}

#[test]
fn queued_play_projects_composition_reward_before_the_triggering_skill() {
    init_config();
    let entity = |uid, skills| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(1),
        current_hp: Some(100),
        ex_point: Some(0),
        skill_group1: skills,
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, vec![100]), entity(20, vec![200, 201])],
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
                    skill_id: Some(100),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(20),
                    skill_id: Some(200),
                    ..Default::default()
                },
                CardInfo {
                    uid: Some(20),
                    skill_id: Some(200),
                    ..Default::default()
                },
            ],
            draw_pile: Vec::new(),
            deck_num: 30,
        }))
        .unwrap();
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
            TargetRequest::self_only(),
        )],
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

    assert_eq!(steps.len(), 4);
    assert_eq!(
        steps[0].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Usecards as i32)
    );
    assert_eq!(
        steps[1].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32)
    );
    assert_eq!(steps[1].act_effect[0].target_id, Some(20));
    assert_eq!(steps[3].act_id, Some(100));
    assert_eq!(managers.ex_point.get(20), 1);
    assert_eq!(managers.card.hand()[0].skill_id, Some(201));
}
