use super::*;
use crate::test_support::frame_contains_skill;

#[test]
fn version_seven_round_start_reads_readiness_before_its_duration_tick() {
    init_config();
    let readiness = |uid, duration, layer| BuffInfo {
        uid: Some(uid),
        buff_id: Some(31210125),
        from_uid: Some(10),
        duration: Some(duration),
        layer: Some(layer),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(100),
                    passive_skill: vec![312101452, 40],
                    buffs: vec![readiness(20, 1, 10), readiness(21, 2, 20)],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    model_id: Some(3149),
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
    let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut expiry = SkillEffectSlot::new(
        ParsedBehavior::new(60291, "AddDevicePower", vec![1, 3]),
        TargetRequest::self_only(),
    );
    expiry.conditions = vec![ParsedCondition {
        opcode: 515005,
        type_name: "BuffIdExpireOnly".into(),
        kind: ParsedConditionKind::BuffExpired(vec![31210125]),
        raw_args: vec!["31210125".into()],
    }];
    expiry.compiled_route = ConditionRoute::compile(&expiry.conditions);
    catalog.insert(ParsedSkillEffect {
        skill_id: 40,
        slots: vec![expiry],
    });
    let mut managers = BattleManagers::seeded(&fight);

    settle_round_start(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 3,
            ..Default::default()
        },
        &[],
    )
    .unwrap();

    assert!(managers.buff.has_buff_id(10, 312101452));
    assert!(managers.buff.snapshot(10, 20).is_none());
    let remaining = managers.buff.snapshot(10, 21).unwrap();
    assert_eq!(remaining.layer, Some(20));
    assert_eq!(remaining.duration, Some(1));
    assert_eq!(managers.conduit.power(1, 1), 3);
}

#[test]
fn version_seven_round_start_event_schedule_runs_each_owners_buff_gate() {
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
                        uid: Some(1000),
                        buff_id: Some(999),
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
    let buff_gate = ParsedCondition {
        opcode: 19103,
        type_name: "HasBuffId".to_owned(),
        kind: ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Present,
            buff_ids: vec![999],
        },
        raw_args: vec!["999".to_owned()],
    };
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 40,
        slots: vec![slot(round_start())],
    });
    catalog.insert(ParsedSkillEffect {
        skill_id: 50,
        slots: vec![slot(buff_gate)],
    });
    catalog.insert(ParsedSkillEffect {
        skill_id: 60,
        slots: vec![slot(round_start())],
    });

    let (round, _, _, _) = run_round_start_after_ai_split(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[],
        0,
    )
    .unwrap();

    let targets = round
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

    assert_eq!(targets, [10, 20, 30]);
    assert_eq!(managers.ex_point.get(10), 1);
    assert_eq!(managers.ex_point.get(20), 1);
    assert_eq!(managers.ex_point.get(30), 1);
}

#[test]
fn version_seven_sync_interleaves_buff_sync_and_late_setup_by_owner() {
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
                        uid: Some(30),
                        buff_id: Some(31270413),
                        from_uid: Some(20),
                        ..Default::default()
                    }],
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
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 40,
        slots: vec![slot(ParsedCondition {
            opcode: 104,
            type_name: "None".to_owned(),
            kind: ParsedConditionKind::None(NoneMode::RoundStart),
            raw_args: Vec::new(),
        })],
    });
    catalog.insert(ParsedSkillEffect {
        skill_id: 50,
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

    let (round, _, _, _) = run_round_start_after_ai_split(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[],
        0,
    )
    .unwrap();
    let targets = round
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

    assert_eq!(targets, [10, 20]);
}

#[test]
fn version_seven_runs_late_setup_before_priority_three_setup() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3149),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ex_point: Some(0),
                    passive_skill: vec![40, 50, 60, 70],
                    buffs: vec![
                        BuffInfo {
                            uid: Some(20),
                            buff_id: Some(2112021),
                            from_uid: Some(10),
                            duration: Some(1),
                            ..Default::default()
                        },
                        BuffInfo {
                            uid: Some(21),
                            buff_id: Some(31270413),
                            from_uid: Some(10),
                            duration: Some(3),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    model_id: Some(3134),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ex_point: Some(0),
                    passive_skill: vec![80],
                    buffs: vec![
                        BuffInfo {
                            uid: Some(30),
                            buff_id: Some(31340003),
                            from_uid: Some(11),
                            ..Default::default()
                        },
                        BuffInfo {
                            uid: Some(31),
                            buff_id: Some(31340008),
                            from_uid: Some(11),
                            ..Default::default()
                        },
                    ],
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
    let mut catalog = crate::engine::skill::effect::catalog::global().clone();
    let slot = |opcode, amount, resource| {
        let mut slot = SkillEffectSlot::new(
            ParsedBehavior::from_spec(resource, vec![amount], Vec::new()),
            TargetRequest::self_only(),
        );
        slot.conditions = vec![ParsedCondition {
            opcode,
            type_name: "None".to_owned(),
            kind: ParsedConditionKind::None(NoneMode::RoundStart),
            raw_args: Vec::new(),
        }];
        slot.compiled_route = ConditionRoute::compile(&slot.conditions);
        slot
    };
    let harmonization = BehaviorSpec::new(60292, "AddDeviceExPoint");
    catalog.insert(ParsedSkillEffect {
        skill_id: 40,
        slots: vec![slot(104, 1, harmonization.clone())],
    });
    catalog.insert(ParsedSkillEffect {
        skill_id: 50,
        slots: vec![slot(10411, 2, harmonization.clone())],
    });
    let mut priority_one = slot(103, 1, harmonization.clone());
    priority_one.conditions.push(ParsedCondition {
        opcode: 57100,
        type_name: "NoBuffId".to_owned(),
        kind: ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Absent,
            buff_ids: vec![2112021],
        },
        raw_args: vec!["2112021".to_owned()],
    });
    priority_one.compiled_route = ConditionRoute::compile(&priority_one.conditions);
    catalog.insert(ParsedSkillEffect {
        skill_id: 60,
        slots: vec![priority_one],
    });
    let mut sync = slot(19104, 1, harmonization);
    sync.conditions[0] = ParsedCondition {
        opcode: 19104,
        type_name: "HasBuffId".to_owned(),
        kind: ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Present,
            buff_ids: vec![31270413],
        },
        raw_args: vec!["31270413".to_owned()],
    };
    sync.compiled_route = ConditionRoute::compile(&sync.conditions);
    catalog.insert(ParsedSkillEffect {
        skill_id: 70,
        slots: vec![sync],
    });
    catalog.insert(ParsedSkillEffect {
        skill_id: 80,
        slots: vec![slot(103, 3, BehaviorSpec::new(20002, "AddExPoint"))],
    });
    let mut managers = BattleManagers::seeded(&fight);
    let gauge_origin = CommandOrigin {
        domain: RuleDomain::Lifecycle,
        key: DefinitionKey::new(0, "TestLingeringGlow"),
    };
    managers
        .execute_gauge(crate::engine::manager::gauge::GaugeCommand::new(
            gauge_origin,
            crate::engine::mechanic::lingering_glow::key(1),
            crate::engine::manager::gauge::GaugeOperation::Enable { max: Some(750) },
        ))
        .unwrap();
    managers
        .execute_gauge(crate::engine::manager::gauge::GaugeCommand::new(
            gauge_origin,
            crate::engine::mechanic::lingering_glow::key(1),
            crate::engine::manager::gauge::GaugeOperation::ChangeValue { delta: 45 },
        ))
        .unwrap();
    managers
        .execute_hp(crate::engine::manager::hp::HpCommand::Kill(
            crate::engine::manager::hp::HpKill {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "TestDefeat"),
                },
                source_uid: 10,
                target_uid: -1,
                config_effect: 1,
            },
        ))
        .unwrap();
    managers
        .execute_card(CardCommand::SetAiQueue(
            crate::engine::manager::card::CardSetAiQueue {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "TestAiQueue"),
                },
                cards: vec![CardInfo {
                    uid: Some(-1),
                    skill_id: Some(100),
                    ..Default::default()
                }],
            },
        ))
        .unwrap();

    let (round, _, _, _) = run_round_start_after_ai_split(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[],
        0,
    )
    .unwrap();
    let deltas = round
        .outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            RuleOutcome::ExPoint(crate::engine::manager::ex_point::ExPointChanges::Value {
                change,
                ..
            }) => Some(change.applied_delta),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(deltas, [1, 3, 1, 1, 2]);
    assert_eq!(managers.ex_point.get(10), 5);
    assert_eq!(managers.ex_point.get(11), 3);
    assert!(!managers.buff.has_buff_id(10, 2112021));
    let root_for_skill = |skill_id| {
        round
            .frames
            .iter()
            .position(|frame| frame_contains_skill(frame, skill_id))
            .expect("round-start skill has a semantic owner")
    };
    let settlement = round
        .frames
        .iter()
        .position(|frame| {
            matches!(
                frame.owner,
                FrameOwner::RoundPhase(RoundPhase::RoundStartSettlement)
            )
        })
        .expect("version seven has a round-start settlement owner");
    let sync = root_for_skill(70);
    let late = root_for_skill(40);
    let priority_three = root_for_skill(50);
    let card_cleanup = round
        .frames
        .iter()
        .position(|frame| {
            matches!(
                frame.owner,
                FrameOwner::RoundPhase(RoundPhase::EntityCardCleanup)
            )
        })
        .expect("version seven has an entity-card-cleanup owner");
    assert_eq!(sync, late);
    assert!(settlement < sync && sync < priority_three);
    assert!(matches!(
        round.frames[sync].owner,
        FrameOwner::RoundPhase(RoundPhase::RoundStartSync)
    ));
    let late_side = round.frames[sync]
        .items
        .iter()
        .find_map(|item| match item {
            FrameItem::Child(child)
                if matches!(child.owner, FrameOwner::SetupSide(_))
                    && frame_contains_skill(child, 40) =>
            {
                Some(child)
            }
            FrameItem::Change(_) | FrameItem::Cue(_) | FrameItem::Child(_) => None,
        })
        .expect("late setup has a side owner inside round-start sync");
    assert!(late_side.items.iter().any(|item| matches!(
        item,
        FrameItem::Child(child)
            if matches!(child.owner, FrameOwner::Skill { skill_id: 40, .. })
    )));
    assert!(matches!(
        round.frames[priority_three].owner,
        FrameOwner::SetupSide(_)
    ));
    let event_children = round.frames[settlement]
        .items
        .iter()
        .filter_map(|item| match item {
            FrameItem::Child(child)
                if frame_contains_skill(child, 60) || frame_contains_skill(child, 80) =>
            {
                Some(child)
            }
            FrameItem::Change(_) | FrameItem::Cue(_) | FrameItem::Child(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(event_children.len(), 2);
    assert!(
        event_children
            .iter()
            .all(|child| frame_contains_skill(child, 60) != frame_contains_skill(child, 80))
    );
    fn contains_change(
        frame: &SemanticFrame,
        predicate: impl Copy + Fn(&BattleChange) -> bool,
    ) -> bool {
        frame.items.iter().any(|item| match item {
            FrameItem::Child(child) => contains_change(child, predicate),
            FrameItem::Change(change) => predicate(change),
            FrameItem::Cue(_) => false,
        })
    }
    assert!(contains_change(
        &round.frames[card_cleanup],
        |change| matches!(change, BattleChange::Card(_))
    ));
    assert!(matches!(
        round.frames[settlement].items.iter().find_map(|item| match item {
            FrameItem::Child(mechanic)
                if matches!(
                    mechanic.trigger,
                    FrameTrigger::Setup {
                        stage: SetupStage::RoundTransitionStart,
                        priority: 3,
                    }
                ) && matches!(mechanic.owner, FrameOwner::SetupMechanic) => Some(mechanic),
            FrameItem::Change(_) | FrameItem::Cue(_) | FrameItem::Child(_) => None,
        }),
        Some(frame)
            if contains_change(frame, |change| matches!(
                change,
                BattleChange::Buff(result)
                    if result.change.added.iter().any(|added| added.buff.buff_id == Some(31340007))
            ))
    ));
    assert_eq!(
        round
            .outcomes
            .iter()
            .filter(|outcome| matches!(
                outcome,
                RuleOutcome::Buff(result)
                    if result.change.added.iter().any(|added| {
                        added.buff.buff_id == Some(31340007)
                            && added.buff.act_info.first().is_some_and(|info| info.param == [22])
                    })
            ))
            .count(),
        2
    );
    assert!(
        round
            .frames
            .iter()
            .all(|frame| !matches!(frame.owner, FrameOwner::SetupMechanic))
    );
    assert!(contains_change(
        &round.frames[settlement],
        |change| matches!(
            change,
            BattleChange::Conduit(
                crate::engine::manager::conduit::ConduitChange::DeviceRestarted { .. }
            )
        )
    ));
    assert!(
        round
            .frames
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != card_cleanup)
            .all(|(_, frame)| {
                !contains_change(frame, |change| matches!(change, BattleChange::Card(_)))
            })
    );
    assert!(
        round
            .frames
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != settlement)
            .all(|(_, frame)| {
                !contains_change(frame, |change| {
                    matches!(
                        change,
                        BattleChange::Conduit(
                            crate::engine::manager::conduit::ConduitChange::DeviceRestarted { .. }
                        )
                    )
                })
            })
    );
}

#[test]
fn round_start_event_setup_preserves_stage_then_owner_order_for_each_layout() {
    init_config();
    for version in [6, 7] {
        let fight = Fight {
            version: Some(version),
            attacker: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(10),
                        team_type: Some(1),
                        current_hp: Some(100),
                        passive_skill: vec![100, 101],
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(11),
                        team_type: Some(1),
                        current_hp: Some(100),
                        passive_skill: vec![200, 201],
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
        let mut catalog = SkillEffectCatalog::default();
        let slot = |condition| {
            let mut slot = SkillEffectSlot::new(
                ParsedBehavior::from_spec(
                    BehaviorSpec::new(20002, "AddExPoint"),
                    vec![1],
                    Vec::new(),
                ),
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
        let transition_start = || ParsedCondition {
            opcode: 45104,
            type_name: "HeroRoundInterval".to_owned(),
            kind: ParsedConditionKind::RoundInterval {
                start_round: 1,
                period: 1,
            },
            raw_args: vec!["1".to_owned(), "1".to_owned()],
        };
        for (skill_id, condition) in [
            (100, round_start()),
            (101, transition_start()),
            (200, round_start()),
            (201, transition_start()),
        ] {
            catalog.insert(ParsedSkillEffect {
                skill_id,
                slots: vec![slot(condition)],
            });
        }
        let mut managers = BattleManagers::seeded(&fight);

        let (round, _, _, _) = run_round_start_after_ai_split(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext {
                current_round: 3,
                ..Default::default()
            },
            &[],
            0,
        )
        .unwrap();

        fn collect_skills(frame: &SemanticFrame, skills: &mut Vec<i32>) {
            if let FrameOwner::Skill { skill_id, .. } = frame.owner
                && [100, 101, 200, 201].contains(&skill_id)
            {
                skills.push(skill_id);
            }
            for item in &frame.items {
                if let FrameItem::Child(child) = item {
                    collect_skills(child, skills);
                }
            }
        }
        let mut skills = Vec::new();
        for frame in &round.frames {
            collect_skills(frame, &mut skills);
        }
        assert_eq!(skills, [100, 200, 101, 201], "version {version}");
    }
}

#[test]
fn round_start_excludes_before_ap_resolution_from_its_phase_buckets() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(0),
                passive_skill: vec![40],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::default();
    let slot = |opcode, mode, amount| {
        let mut slot = SkillEffectSlot::new(
            ParsedBehavior::from_spec(
                BehaviorSpec::new(20002, "AddExPoint"),
                vec![amount],
                Vec::new(),
            ),
            TargetRequest::self_only(),
        );
        slot.conditions = vec![ParsedCondition {
            opcode,
            type_name: "None".to_owned(),
            kind: ParsedConditionKind::None(mode),
            raw_args: Vec::new(),
        }];
        slot.compiled_route = ConditionRoute::compile(&slot.conditions);
        slot
    };
    catalog.insert(ParsedSkillEffect {
        skill_id: 40,
        slots: vec![
            slot(107, NoneMode::BeforeApResolve, 1),
            slot(105, NoneMode::AfterRoundStart, 2),
        ],
    });
    let mut managers = BattleManagers::seeded(&fight);

    let (fight_steps, next_round_begin_steps) = run_round_start_split(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        1,
    )
    .unwrap();
    assert_eq!(managers.ex_point.get(10), 2);
    fn contains_ex_point_change(frame: &crate::engine::runtime::record::SemanticFrame) -> bool {
        frame.items.iter().any(|item| match item {
            crate::engine::runtime::record::FrameItem::Change(change) => {
                matches!(
                    change.as_ref(),
                    crate::engine::runtime::change::BattleChange::ExPoint(_)
                )
            }
            crate::engine::runtime::record::FrameItem::Child(child) => {
                contains_ex_point_change(child)
            }
            crate::engine::runtime::record::FrameItem::Cue(_) => false,
        })
    }
    let after_round_start = next_round_begin_steps
        .frames
        .iter()
        .position(contains_ex_point_change)
        .expect("after-round-start change is recorded");
    let layer_halo_sync = next_round_begin_steps
        .frames
        .iter()
        .position(|frame| {
            frame.items.iter().any(|item| {
                matches!(
                    item,
                    crate::engine::runtime::record::FrameItem::Cue(
                        crate::engine::runtime::record::RoundCue::LayerHaloSync { .. }
                    )
                )
            })
        })
        .expect("layer-halo synchronization is recorded");
    assert!(after_round_start < layer_halo_sync);
    let fight_steps = crate::engine::packet::timeline::project(&fight_steps.frames).unwrap();
    let next_round_begin_steps =
        crate::engine::packet::timeline::project(&next_round_begin_steps.frames).unwrap();
    fn collect_ex_point_changes(effects: &[sonettobuf::ActEffect], changes: &mut Vec<i32>) {
        for effect in effects {
            if effect.effect_type
                == Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32)
                && let Some(amount) = effect.effect_num
            {
                changes.push(amount);
            }
            if let Some(step) = &effect.fight_step {
                collect_ex_point_changes(&step.act_effect, changes);
            }
        }
    }
    let ex_point_changes = |steps: &[sonettobuf::FightStep]| {
        let mut changes = Vec::new();
        for step in steps {
            collect_ex_point_changes(&step.act_effect, &mut changes);
        }
        changes
    };

    assert!(ex_point_changes(&fight_steps).is_empty());
    assert_eq!(ex_point_changes(&next_round_begin_steps), vec![2]);
    assert_eq!(
        fight_steps[0].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Changeround as i32)
    );
    assert_eq!(
        next_round_begin_steps[0].act_effect[0].effect_type,
        Some(sonettobuf::effect_type_enum::EffectType::Dealcard1 as i32)
    );
    let final_effects = &next_round_begin_steps.last().unwrap().act_effect;
    assert_eq!(
        final_effects
            .iter()
            .map(|effect| effect.effect_type.unwrap())
            .collect::<Vec<_>>(),
        vec![
            sonettobuf::effect_type_enum::EffectType::Cardspush as i32,
            sonettobuf::effect_type_enum::EffectType::Carddecknum as i32,
        ]
    );
}

#[test]
fn round_start_card_event_advances_buffs_added_late_in_round_start() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: [(10, 1010, vec![40]), (20, 1020, Vec::new())]
                .into_iter()
                .map(|(uid, buff_uid, passive_skill)| FightEntityInfo {
                    uid: Some(uid),
                    team_type: Some(1),
                    current_hp: Some(100),
                    passive_skill,
                    buffs: vec![BuffInfo {
                        uid: Some(buff_uid),
                        buff_id: Some(31390132),
                        from_uid: Some(uid),
                        duration: Some(1),
                        ..Default::default()
                    }],
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(BehaviorSpec::new(1, "AddBuff"), vec![30810116], Vec::new()),
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
    let mut managers = BattleManagers::seeded(&fight);

    let (_, next_round) = run_round_start_split(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        1,
    )
    .unwrap();

    assert!(!managers.buff.has_buff_id(10, 30810116));
    assert!(!managers.buff.has_buff_id(10, 31390132));
    assert!(!managers.buff.has_buff_id(20, 31390132));
    let steps =
        crate::engine::packet::timeline::project_for_version(&next_round.frames, 7).unwrap();
    let expiry_groups = steps
        .iter()
        .map(|step| {
            step.act_effect
                .iter()
                .filter(|effect| {
                    effect.effect_type
                        == Some(sonettobuf::effect_type_enum::EffectType::Buffdel as i32)
                })
                .filter_map(|effect| Some((effect.target_id?, effect.buff.as_ref()?.buff_id?)))
                .collect::<Vec<_>>()
        })
        .filter(|effects| !effects.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(
        expiry_groups,
        vec![vec![(10, 30810116)], vec![(10, 31390132), (20, 31390132)]]
    );
    let round_start_card = next_round
        .events
        .iter()
        .position(|event| matches!(event, BattleEvent::Kind(EventKind::RoundStartCard)))
        .expect("round-start-card event");
    let expired = next_round
        .events
        .iter()
        .position(
            |event| matches!(event, BattleEvent::BuffRemoved(change) if change.buff_id == 30810116),
        )
        .expect("duration expiry");
    assert!(round_start_card < expired);
    assert_eq!(
        next_round
            .events
            .iter()
            .filter(|event| matches!(event, BattleEvent::Kind(EventKind::RoundStartCard)))
            .count(),
        1
    );
}

#[test]
fn round_start_card_event_runs_buff_gated_skill_rules() {
    init_config();
    let (fight, catalog) = buff_gated_generic_temp_card_fixture();
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);

    let (_, next_round) = run_round_start_split(
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
    assert_buff_gated_generic_temp_card(&managers, &next_round);
}

#[test]
fn round_start_removes_hp_restored_above_the_dissonance_value() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                position: Some(1),
                team_type: Some(1),
                current_hp: Some(900),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(1000),
                    buff_id: Some(108_300_002),
                    from_uid: Some(0),
                    layer: Some(1),
                    act_info: vec![sonettobuf::BuffActInfo {
                        act_id: Some(1059),
                        param: vec![700],
                        str_param: Some(String::new()),
                    }],
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
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    let (round, _, _, _) = run_round_start_after_ai_split(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &[],
        0,
    )
    .unwrap();

    assert_eq!(managers.hp.current(10), 700);
    let trims = round
        .outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            RuleOutcome::Hp(execution) => execution.changes.hp,
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(trims.len(), 1);
    assert_eq!((trims[0].before, trims[0].after), (900, 700));
}
