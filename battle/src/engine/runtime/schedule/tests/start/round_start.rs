use super::*;

#[test]
fn version_seven_advances_round_start_condition_grants_at_the_same_settlement() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3092),
                    team_type: Some(1),
                    current_hp: Some(100),
                    passive_skill: vec![30920103],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    model_id: Some(3121),
                    team_type: Some(1),
                    current_hp: Some(100),
                    passive_skill: vec![40],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut later_grant = SkillEffectSlot::new(
        ParsedBehavior::new(1, "AddBuff", vec![30920107]),
        TargetRequest::self_only(),
    );
    later_grant.conditions = vec![ParsedCondition {
        opcode: 102,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(NoneMode::RoundStart),
        raw_args: Vec::new(),
    }];
    later_grant.compiled_route = ConditionRoute::compile(&later_grant.conditions);
    catalog.insert(ParsedSkillEffect {
        skill_id: 40,
        slots: vec![later_grant],
    });
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();
    let marker = |managers: &BattleManagers| {
        managers
            .buff
            .active_for(10)
            .find(|buff| buff.buff_id == Some(30920104))
            .map(|buff| (buff.uid.unwrap(), buff.duration.unwrap()))
            .unwrap()
    };
    let later_duration = |managers: &BattleManagers| {
        managers
            .buff
            .active_for(11)
            .find(|buff| buff.buff_id == Some(30920107))
            .and_then(|buff| buff.duration)
    };

    run_start(
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
    let (opening_uid, duration) = marker(&managers);
    assert_eq!(duration, 1);
    assert_eq!(later_duration(&managers), Some(2));

    settle_round_start(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext {
            current_round: 2,
            ..Default::default()
        },
        &[],
    )
    .unwrap();
    let (renewed_uid, duration) = marker(&managers);
    assert_ne!(opening_uid, renewed_uid);
    assert_eq!(duration, 1);
    assert_eq!(later_duration(&managers), Some(2));
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
fn configured_round_after_runs_for_defenders_and_advances_mane_duration() {
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

    run_before_ai_round_start(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 2,
            ..Default::default()
        },
        2,
        &[],
        None,
    )
    .unwrap();
    for uid in [-1, -2, -3] {
        assert_eq!(
            managers
                .buff
                .active_for(uid)
                .find(|buff| buff.buff_id == Some(116362001))
                .and_then(|buff| buff.duration),
            Some(3),
            "uid={uid}"
        );
    }
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
