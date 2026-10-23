use super::*;

#[test]
fn round_start_executes_each_team_at_its_own_turn_boundary() {
    init_config();
    let entity = |uid, team_type, skill_id| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(100),
        ex_point: Some(0),
        passive_skill: vec![skill_id],
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 40)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, 50)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::default();
    for skill_id in [40, 50] {
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
            skill_id,
            slots: vec![slot],
        });
    }
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();

    run_before_ai_round_start(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
        1,
        &[],
        None,
    )
    .unwrap();
    assert_eq!(managers.ex_point.get(10), 0);
    assert_eq!(managers.ex_point.get(-1), 1);

    let (attacker_round, _, _, _) = run_round_start_after_ai_split(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
        &[],
        3,
    )
    .unwrap();
    assert_eq!(managers.ex_point.get(10), 1);
    assert_eq!(managers.ex_point.get(-1), 1);

    let settlement = attacker_round
        .frames
        .iter()
        .find(|frame| {
            matches!(
                frame.owner,
                FrameOwner::RoundPhase(RoundPhase::RoundStartSettlement)
            )
        })
        .expect("attacker round start has a settlement phase");
    let setup_side = settlement
        .items
        .iter()
        .find_map(|item| match item {
            FrameItem::Child(frame)
                if matches!(frame.owner, FrameOwner::SetupSide(SetupSide::Attacker)) =>
            {
                Some(frame.as_ref())
            }
            _ => None,
        })
        .expect("round-start setup is owned by the attacker side");
    assert!(matches!(
        setup_side.items.as_slice(),
        [FrameItem::Child(skill)]
            if matches!(skill.owner, FrameOwner::Skill {
                source_uid: 10,
                skill_id: 40,
                ..
            })
    ));
}

#[test]
fn version_seven_defers_defender_round_start_until_after_ai() {
    let entity = |skill_ids| FightEntityInfo {
        uid: Some(-1),
        team_type: Some(2),
        current_hp: Some(100),
        ex_point: Some(0),
        passive_skill: skill_ids,
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        defender: Some(FightTeam {
            entitys: vec![entity(vec![50, 60])],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut round_start = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![10], Vec::new()),
        TargetRequest::self_only(),
    );
    round_start.conditions = vec![ParsedCondition {
        opcode: 100,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(NoneMode::RoundStart),
        raw_args: Vec::new(),
    }];
    round_start.compiled_route = ConditionRoute::compile(&round_start.conditions);
    let mut conditional = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
        TargetRequest::self_only(),
    );
    conditional.conditions = vec![ParsedCondition {
        opcode: 750101,
        type_name: "PlayerHasBuff".to_owned(),
        kind: ParsedConditionKind::PlayerBuffPresence {
            present: false,
            buff_id: 109320002,
        },
        raw_args: vec!["2".to_owned(), "0".to_owned(), "109320002".to_owned()],
    }];
    conditional.compiled_route = ConditionRoute::compile(&conditional.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 50,
        slots: vec![round_start],
    });
    catalog.insert(ParsedSkillEffect {
        skill_id: 60,
        slots: vec![conditional],
    });
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);

    run_before_ai_round_start(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        2,
        &[],
        None,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(-1), 1);

    run_round_start_after_ai_split(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[],
        3,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(-1), 5);
}

#[test]
fn defender_action_phase_runs_progress_reactions_before_ai() {
    let entity = |uid, team_type, skill_id| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(100),
        ex_point: Some(0),
        passive_skill: vec![skill_id],
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 40)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, 50)],
            ..Default::default()
        }),
        progress_list: vec![sonettobuf::FightProgressInfo {
            id: Some(8),
            max: Some(100),
            value: Some(100),
            show_id: Some(1),
            skill_id: Some(1),
        }],
        ..Default::default()
    };
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 1010301,
        type_name: "ProgressValue".to_owned(),
        kind: ParsedConditionKind::FightProgressValue {
            progress_id: 8,
            compare_code: 3,
            threshold: 100,
        },
        raw_args: vec!["8".to_owned(), "3".to_owned(), "100".to_owned()],
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    for skill_id in [40, 50] {
        catalog.insert(ParsedSkillEffect {
            skill_id,
            slots: vec![slot.clone()],
        });
    }
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);

    let result = run_before_ai_round_start(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        2,
        &[],
        None,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 0);
    assert_eq!(managers.ex_point.get(-1), 1);
    assert!(
        result
            .events
            .iter()
            .any(|event| { event.kind() == EventKind::DefenderActionPhaseStart })
    );
}

#[test]
fn defender_round_start_late_applies_configured_buffs_before_ai_and_filters_owners() {
    init_config();
    let entity = |uid, current_hp| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(2),
        current_hp: Some(current_hp),
        ex_point: Some(5),
        passive_skill: vec![1163855066],
        attr: Some(HeroAttribute {
            hp: Some(100),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    ex_skill: Some(999),
                    ..entity(-1, 100)
                },
                entity(-2, 50),
                entity(-3, 0),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    catalog.insert(ParsedSkillEffect {
        skill_id: 999,
        slots: Vec::new(),
    });
    catalog.insert_damage_rate(999, 1_000);
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();

    run_before_ai_round_start(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext {
            current_round: 2,
            ..Default::default()
        },
        1,
        &[],
        None,
    )
    .unwrap();

    for buff_id in [5081, 23501] {
        assert!(managers.buff.has_buff_id(-1, buff_id));
        assert!(!managers.buff.has_buff_id(-2, buff_id));
        assert!(!managers.buff.has_buff_id(-3, buff_id));
    }

    run_ai_actions(
        &fight,
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext {
            current_round: 2,
            ..Default::default()
        },
        [AiSkillChoice {
            source_uid: -1,
            skill_id: 999,
            target_uid: 10,
        }],
        &[],
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(-1), 0);
    assert_eq!(managers.hp.current(10), 8_800);
}

#[test]
fn round_start_zero_reads_timed_stacks_before_duration_advances() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(100),
                    passive_skill: vec![40],
                    buffs: vec![
                        BuffInfo {
                            uid: Some(1),
                            buff_id: Some(31050111),
                            duration: Some(1),
                            layer: Some(27),
                            from_uid: Some(10),
                            ..Default::default()
                        },
                        BuffInfo {
                            uid: Some(2),
                            buff_id: Some(31050111),
                            duration: Some(2),
                            layer: Some(6),
                            from_uid: Some(10),
                            ..Default::default()
                        },
                    ],
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
        ..Default::default()
    };
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(
            BehaviorSpec::new(60124, "AddBuffByBuffLayerRange"),
            Vec::new(),
            vec![
                "31050111".into(),
                "31050141,31050142,31050143".into(),
                "5,15,25,100".into(),
            ],
        ),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 102,
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
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.buff.add(&managers.hp, 10, 10, 31050142, 0);
    let old_lower_field_uid = managers.buff.buff_id_uid(10, 31050142).unwrap();
    let old_lower_halo_uid = managers.buff.buff_id_uid(11, 31050145).unwrap();
    managers.buff.add(&managers.hp, 10, 10, 31050143, 0);
    let old_field_uid = managers.buff.buff_id_uid(10, 31050143).unwrap();
    let old_halo_uid = managers.buff.buff_id_uid(11, 31050146).unwrap();

    run_round_start_split(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        1,
    )
    .unwrap();

    assert!(managers.buff.has_buff_id(10, 31050143));
    assert!(!managers.buff.has_buff_id(10, 31050141));
    assert!(!managers.buff.has_buff_id(10, 31050142));
    assert!(!managers.buff.has_buff_id(11, 31050145));
    assert_eq!(managers.buff.snapshot(10, old_lower_field_uid), None);
    assert_eq!(managers.buff.snapshot(11, old_lower_halo_uid), None);
    assert_ne!(managers.buff.buff_id_uid(10, 31050143), Some(old_field_uid));
    assert_ne!(managers.buff.buff_id_uid(11, 31050146), Some(old_halo_uid));
}

#[test]
fn defender_round_start_groups_event_subscribers_with_round_start_setup() {
    init_config();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(100),
                ex_point: Some(0),
                passive_skill: vec![50, 60],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut event_slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
        TargetRequest::self_only(),
    );
    event_slot.conditions = vec![ParsedCondition {
        opcode: 57104,
        type_name: "NoBuffId".to_owned(),
        kind: ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Absent,
            buff_ids: vec![99],
        },
        raw_args: vec!["99".to_owned()],
    }];
    event_slot.compiled_route = ConditionRoute::compile(&event_slot.conditions);
    let mut setup_slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
        TargetRequest::self_only(),
    );
    setup_slot.conditions = vec![ParsedCondition {
        opcode: 103,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(NoneMode::RoundStart),
        raw_args: Vec::new(),
    }];
    setup_slot.compiled_route = ConditionRoute::compile(&setup_slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 50,
        slots: vec![event_slot],
    });
    catalog.insert(ParsedSkillEffect {
        skill_id: 60,
        slots: vec![setup_slot],
    });
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);

    let result = run_before_ai_round_start(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 1,
            ..Default::default()
        },
        1,
        &[],
        None,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(-1), 2);
    assert_eq!(result.frames.len(), 1);
    let SemanticFrame {
        owner: FrameOwner::RoundPhase(RoundPhase::RoundStartEvent),
        items,
        ..
    } = &result.frames[0]
    else {
        panic!("round start must own the grouped event")
    };
    let [FrameItem::Child(event)] = items.as_slice() else {
        panic!("round start must contain one event frame")
    };
    assert!(matches!(
        event.as_ref(),
        SemanticFrame {
            owner: FrameOwner::EventRule,
            trigger: FrameTrigger::Event(BattleEvent::RoundStart),
            ..
        }
    ));
    let skill_ids = event
        .items
        .iter()
        .filter_map(|item| match item {
            FrameItem::Child(frame) => match frame.owner {
                FrameOwner::Skill { skill_id, .. } => Some(skill_id),
                _ => None,
            },
            FrameItem::Change(_) | FrameItem::Cue(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(skill_ids, vec![50, 60]);
}

#[test]
fn round_start_runs_buff_sync_rules_again_for_the_new_round() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(0),
                passive_skill: vec![40],
                buffs: vec![BuffInfo {
                    uid: Some(1),
                    buff_id: Some(99),
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
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 19104,
        type_name: "HasBuffId".to_owned(),
        kind: ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Present,
            buff_ids: vec![99],
        },
        raw_args: vec!["99".to_owned()],
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 40,
        slots: vec![slot],
    });
    let mut managers = BattleManagers::seeded(&fight);

    run_round_start_split(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        1,
    )
    .unwrap();

    assert_eq!(managers.ex_point.get(10), 1);
}
