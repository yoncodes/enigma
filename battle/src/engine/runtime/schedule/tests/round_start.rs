use super::*;
use crate::engine::runtime::record::SetupSide;

#[test]
fn round_start_settles_unlisted_capacity_owner_before_its_after_settlement_act() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(99),
                team_type: Some(1),
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
    managers.buff.add(&managers.hp, 99, 99, 31250151, 0);
    managers.buff.add(&managers.hp, 99, 99, 31260121, 0);
    let features = managers.buff.active_features(&managers.hp);
    let raspberry = features
        .iter()
        .find(|feature| buff_act::is_kind(feature, buff_act::registry::BuffActKind::Raspberry))
        .unwrap();
    let capacity_groups = crate::engine::mechanic::shadow_cloak::capacity_rule_groups(
        &managers,
        &features,
        &std::collections::HashMap::from([((99, raspberry.buff_uid), 100)]),
    )
    .unwrap();
    let plan = RoundStartSettlementPlan::new(capacity_groups, vec![10]);

    let result = run_round_start_owner_settlement(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        plan,
    )
    .unwrap();

    let capacity = result
        .outcomes
        .iter()
        .position(|outcome| matches!(outcome, RuleOutcome::RaspberryCapacity(_)))
        .unwrap();
    let after_settlement_loss = result
        .outcomes
        .iter()
        .position(|outcome| {
            matches!(
                outcome,
                RuleOutcome::Hp(execution)
                    if execution
                        .changes
                        .hp
                        .is_some_and(|hp| hp.target_uid == 99 && hp.delta < 0)
            )
        })
        .unwrap();
    assert!(capacity < after_settlement_loss);
}

#[test]
fn round_start_capacity_uses_only_exact_raspberry_loss_instances() {
    use crate::engine::manager::hp::{HpChange, HpChanges, HurtDamageFromType, HurtInfoData};

    let hp_outcome = |domain, key, delta| {
        RuleOutcome::Hp(Box::new(crate::engine::manager::HpExecution {
            changes: HpChanges {
                origin: CommandOrigin { domain, key },
                source_uid: 10,
                target_uid: 10,
                damage: None,
                team_shared_shield_absorbed: None,
                team_shared_shield_removed: None,
                shield_absorbed: None,
                shield_granted: None,
                max_hp: None,
                hp: Some(HpChange {
                    target_uid: 10,
                    before: 100,
                    delta,
                    after: 100 + delta,
                    max: 100,
                    config_effect: 0,
                    hurt: Some(HurtInfoData {
                        from_uid: 10,
                        is_crit: false,
                        career_restraint: false,
                        reduce_hp: 0,
                        effect_id: 0,
                        skill_id: 0,
                        damage_from: HurtDamageFromType::Buff,
                        buff_act_id: 1042,
                        buff_uid: 20,
                        hurt_effect_type: 0,
                        display_amount: None,
                    }),
                    assassinate: false,
                    effect_type: 0,
                    display_amount: None,
                }),
                toughness: None,
                kill: None,
                death: None,
            },
            indicator: None,
        }))
    };
    let result = DrainResult {
        outcomes: vec![
            hp_outcome(
                RuleDomain::BuffAct,
                DefinitionKey::new(1042, "Raspberry"),
                -10,
            ),
            hp_outcome(
                RuleDomain::Behavior,
                DefinitionKey::new(1042, "Raspberry"),
                -50,
            ),
            hp_outcome(
                RuleDomain::BuffAct,
                DefinitionKey::new(1041, "RaspberryBigSkill"),
                -60,
            ),
            hp_outcome(
                RuleDomain::BuffAct,
                DefinitionKey::new(1042, "Raspberry"),
                -15,
            ),
        ],
        ..Default::default()
    };

    assert_eq!(
        raspberry_losses(&result),
        std::collections::HashMap::from([((10, 20), 25)])
    );
}

#[test]
fn defender_round_start_expires_the_previous_status_before_alternating_setup() {
    init_config();
    let entity = |uid, model_id, passive_skill, buffs| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(100),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        passive_skill,
        buffs,
        ..Default::default()
    };
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![
                entity(
                    -2,
                    900016101,
                    vec![811453],
                    vec![BuffInfo {
                        uid: Some(1076),
                        buff_id: Some(2112021),
                        from_uid: Some(-2),
                        duration: Some(1),
                        ..Default::default()
                    }],
                ),
                entity(-3, 900016102, Vec::new(), Vec::new()),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);

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
    )
    .unwrap();

    assert!(!managers.buff.has_buff_id(-2, 2112021));
    let active = managers
        .buff
        .active_for(-3)
        .filter(|buff| buff.buff_id == Some(2112021))
        .collect::<Vec<_>>();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].from_uid, Some(-2));
}

#[test]
fn round_start_resolves_field_with_configured_allied_threshold_modifiers() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    team_type: Some(1),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(30),
                        buff_id: Some(31280117),
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
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(50019, "AddMagicCircle"),
    };
    let thresholds =
        crate::engine::skill::behavior::magic_circle::field_thresholds(30001, 1, &managers);
    managers
        .execute_field(FieldCommand {
            origin,
            team: 1,
            operation: FieldOperation::DeployIfAbsent {
                definition: FieldDefinition {
                    field_id: 30001,
                    duration: 3,
                },
                create_uid: 10,
                initial_level: 1,
                thresholds,
            },
        })
        .unwrap();
    managers
        .execute_field(FieldCommand {
            origin,
            team: 1,
            operation: FieldOperation::ChangeProgress { delta: 80 },
        })
        .unwrap();

    let (first_round, _) = run_round_start_split(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        1,
    )
    .unwrap();
    assert!(first_round.frames.iter().any(|frame| {
        matches!(
            frame.items.as_slice(),
            [crate::engine::runtime::record::FrameItem::Cue(
                RoundCue::ChangeRound { .. }
            )]
        )
    }));
    assert_eq!(managers.field.get(1).unwrap().level, 1);

    managers
        .execute_field(FieldCommand {
            origin,
            team: 1,
            operation: FieldOperation::ChangeProgress { delta: 20 },
        })
        .unwrap();
    run_round_start_split(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        2,
    )
    .unwrap();
    let field = managers.field.get(1).unwrap();
    assert_eq!(field.level, 3);
    assert_eq!(field.definition.field_id, 30003);
    assert_eq!(field.next_upgrade_progress, 120);
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
fn round_start_generated_cards_exist_before_card_energy_allocation() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ex_point: Some(10),
                ex_point_type: Some(3),
                buffs: vec![
                    BuffInfo {
                        uid: Some(20),
                        buff_id: Some(312451407),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(21),
                        buff_id: Some(2240000),
                        from_uid: Some(10),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let features = managers.buff.active_features(&managers.hp);
    let enable = enable_rule_ops(&managers.gauge, &features, 10)
        .pop()
        .unwrap();
    for op in [enable.team_energy, enable.inspiration] {
        let RuleOp::Command(BattleCommand::Gauge(command)) = op else {
            panic!("impromptu enable emits gauge commands");
        };
        managers.execute_gauge(command).unwrap();
    }
    let tag = features
        .iter()
        .find(|feature| buff_act::is_kind(feature, buff_act::registry::BuffActKind::EmitterTag))
        .unwrap();
    managers
        .execute_gauge(crate::engine::manager::gauge::GaugeCommand::new(
            buff_act::feature_command_origin(tag).unwrap(),
            team_energy_key(1),
            crate::engine::manager::gauge::GaugeOperation::ChangeValue { delta: 1 },
        ))
        .unwrap();
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_card_energy_snapshot(vec![CardInfo {
        uid: Some(10),
        skill_id: Some(312451035),
        energy: Some(1),
        ..Default::default()
    }]);

    let (_, next_round) = run_round_start_split(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut determinism,
        TargetContext::default(),
        1,
    )
    .unwrap();
    let steps = crate::engine::packet::timeline::project(&next_round.frames).unwrap();
    let reset = steps
        .iter()
        .position(|step| {
            step.act_effect.iter().any(|effect| {
                effect.fight_step.as_ref().is_some_and(|nested| {
                    nested.act_effect.iter().any(|effect| {
                        effect.effect_type
                            == Some(sonettobuf::effect_type_enum::EffectType::Expointchange as i32)
                            && effect.effect_num == Some(-10)
                    })
                })
            })
        })
        .unwrap();
    let allocation = steps
        .iter()
        .position(|step| {
            step.act_effect.iter().any(|effect| {
                effect.effect_type
                    == Some(sonettobuf::effect_type_enum::EffectType::Allocatecardenergy as i32)
                    && effect
                        .card_info_list
                        .iter()
                        .any(|card| card.skill_id == Some(312451035))
            })
        })
        .unwrap();

    assert!(reset < allocation);
    assert_eq!(managers.ex_point.get(10), 0);
}

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
fn round_start_zero_reads_timed_stacks_before_duration_advances() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                passive_skill: vec![40],
                buffs: vec![
                    BuffInfo {
                        uid: Some(1),
                        buff_id: Some(31050111),
                        duration: Some(1),
                        layer: Some(10),
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
            }],
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

    run_round_start_split(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        1,
    )
    .unwrap();

    assert!(managers.buff.has_buff_id(10, 31050142));
    assert!(!managers.buff.has_buff_id(10, 31050141));
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
