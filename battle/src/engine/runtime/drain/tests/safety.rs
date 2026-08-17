use super::*;

fn queued(op: RuleOp) -> QueuedOp {
    QueuedOp {
        op,
        trigger: SkillOpTrigger::Active,
        skill_execution: None,
        frame_path: None,
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber_owner_uid: None,
    }
}

#[test]
fn repeated_capped_state_commands_commit_cumulative_absolute_markers() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3146),
                current_hp: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(1006),
                    buff_id: Some(31460143),
                    from_uid: Some(10),
                    act_info: vec![sonettobuf::BuffActInfo {
                        act_id: Some(1139),
                        param: vec![70_000],
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
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let command = RuleOp::Command(BattleCommand::Buff(BuffCommand::AccumulateCappedActState(
        crate::engine::manager::buff::BuffAccumulateCappedActState {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60298, "AddMeiLeiErCharge"),
            },
            target_uid: 10,
            buff_uid: 1006,
            act_id: 1139,
            delta: 40_000,
            maximum: 150_000,
        },
    )));
    let mut queue = VecDeque::from([queued(command.clone()), queued(command)]);

    let result = drain_queue_with_frames(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &mut queue,
        Vec::new(),
    )
    .unwrap();

    let markers = result
        .outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            RuleOutcome::Buff(changes) => changes.act_info_markers.first(),
            _ => None,
        })
        .map(|marker| marker.params[0])
        .collect::<Vec<_>>();
    assert_eq!(markers, vec![110_000, 150_000]);
    assert_eq!(
        managers.buff.snapshot(10, 1006).unwrap().act_info[0].param,
        [150_000]
    );
}

fn entity(uid: i64, model_id: i32, hp: i32, passive_skill: Vec<i32>) -> FightEntityInfo {
    FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(hp),
        ex_point: Some(0),
        passive_skill,
        attr: Some(HeroAttribute {
            hp: Some(hp),
            attack: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn dead_slot(behavior: ParsedBehavior) -> SkillEffectSlot {
    let mut slot = SkillEffectSlot::new(behavior, TargetRequest::self_only());
    slot.conditions = vec![ParsedCondition {
        opcode: 812,
        type_name: "Dead".to_owned(),
        kind: crate::engine::skill::condition::registry::parse(812, "Dead", &[]).unwrap(),
        raw_args: Vec::new(),
    }];
    slot.compiled_route = ConditionRoute::compile(&slot.conditions);
    slot
}

fn death_reaction_result(heal: bool) -> (BattleManagers, DrainResult) {
    crate::test_support::init_config();
    let fight = Fight {
        battle_id: Some(301110),
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 100, Vec::new())],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 30111005, 1, vec![300])],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut slots = vec![dead_slot(ParsedBehavior::from_spec(
        BehaviorSpec::new(20002, "AddExPoint"),
        vec![1],
        Vec::new(),
    ))];
    if heal {
        slots.push(dead_slot(ParsedBehavior::from_spec(
            BehaviorSpec::new(20001, "Heal"),
            vec![1],
            vec!["1".into()],
        )));
    }
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 300,
        slots,
    });
    catalog.insert(ParsedSkillEffect {
        skill_id: 200,
        slots: Vec::new(),
    });
    catalog.insert_damage_rate(200, 1_000);
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut attack: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 200,
    }
    .into();
    attack.mode = SkillExecutionMode::Active;
    attack.target = SkillTarget::Explicit(-1);
    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            battle_id: 301110,
            ..Default::default()
        },
        [RuleOp::Skill(attack)],
    )
    .unwrap();
    (managers, result)
}

#[test]
fn drain_budget_rejects_unbounded_work_and_recursion() {
    let mut budget = DrainBudget {
        remaining_operations: 1,
    };

    assert_eq!(budget.consume(0), Ok(()));
    assert_eq!(budget.consume(0), Err(DrainError::OperationLimitExceeded));
    assert_eq!(
        DrainBudget {
            remaining_operations: 1,
        }
        .consume(MAX_DRAIN_DEPTH),
        Err(DrainError::RecursionLimitExceeded)
    );
}

#[test]
fn drain_state_preserves_deferred_order_and_empty_release_points() {
    let mut state = DrainState::new(TargetContext::default());
    let action_path = vec![0];
    let skill = |skill_id| {
        queued(RuleOp::Skill(
            SkillRequest {
                source_uid: 10,
                skill_id,
            }
            .into(),
        ))
    };

    state.defer_after_hit(Some(&action_path), vec![skill(1)]);
    state.defer_after_hit(Some(&action_path), vec![skill(2)]);
    let skill_ids = state
        .take_after_hit(Some(&action_path))
        .unwrap()
        .into_iter()
        .map(|queued| match queued.op {
            RuleOp::Skill(invocation) => invocation.plan.skill_id,
            _ => unreachable!("test queue contains only skill invocations"),
        })
        .collect::<Vec<_>>();
    assert_eq!(skill_ids, vec![2, 1]);

    state.defer_after_hit(Some(&action_path), Vec::new());
    assert_eq!(
        state
            .take_after_hit(Some(&action_path))
            .map(|queued| queued.len()),
        Some(0)
    );
}

#[test]
fn nested_drain_restores_depth_after_error() {
    let fight = Fight::default();
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();
    let mut result = DrainResult::default();
    let mut state = DrainState::new(TargetContext::default());

    let error = drain_nested_queue(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut determinism,
        VecDeque::from([queued(RuleOp::ModifyActiveSkillTargets {
            additional_count: 1,
        })]),
        &mut result,
        &mut state,
    )
    .unwrap_err();

    assert!(matches!(error, DrainError::MissingActiveSkillContext));
    assert_eq!(state.depth(), 0);
}

#[test]
fn death_settlement_restores_depth_after_nested_error() {
    let fight = Fight::default();
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut result = DrainResult::default();
    let mut state = DrainState::new(TargetContext::default());

    let error = drain_death_settlement_queue(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        VecDeque::from([queued(RuleOp::ModifyActiveSkillTargets {
            additional_count: 1,
        })]),
        &mut result,
        &mut state,
    )
    .unwrap_err();

    assert!(matches!(error, DrainError::MissingActiveSkillContext));
    assert_eq!(state.depth(), 0);
    assert!(!state.death_settlement_in_progress());
}

#[test]
fn missing_buff_act_definition_preserves_exact_runtime_identity() {
    assert_eq!(
        required_buff_act_definition(Some(999_999), "MissingExactType").unwrap_err(),
        DrainError::MissingBuffActDefinition {
            opcode: Some(999_999),
            type_name: "MissingExactType".to_owned(),
        }
    );
}

#[test]
fn dead_passive_nonheal_then_heal_prevents_terminal() {
    let (managers, result) = death_reaction_result(true);

    let resource = result
        .events
        .iter()
        .position(|event| {
            matches!(
                event,
                BattleEvent::ExPointChanged(change) if change.source_uid == -1
            )
        })
        .unwrap();
    let heal = result
        .events
        .iter()
        .position(|event| {
            matches!(
                event,
                BattleEvent::HpHealed {
                    source_uid: -1,
                    target_uid: -1,
                    amount: 1,
                    ..
                }
            )
        })
        .unwrap();
    assert!(resource < heal);
    assert_eq!(managers.hp.current(-1), 1);
    assert!(
        !result
            .events
            .iter()
            .any(|event| matches!(event, BattleEvent::BattleTerminalCommitted { .. }))
    );
}

#[test]
fn dead_passive_without_heal_commits_terminal_once() {
    let (managers, result) = death_reaction_result(false);

    assert_eq!(managers.hp.current(-1), 0);
    assert!(result.events.iter().any(|event| matches!(
        event,
        BattleEvent::ExPointChanged(change) if change.source_uid == -1
    )));
    assert_eq!(
        result
            .events
            .iter()
            .filter(|event| matches!(event, BattleEvent::BattleTerminalCommitted { .. }))
            .count(),
        1
    );
}

#[test]
fn terminal_skips_unstarted_losing_subscribers_by_presentation_frame_owner() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 100, Vec::new())],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                entity(-1, 2, 100, Vec::new()),
                entity(-2, 3, 100, Vec::new()),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let trigger = SkillOpTrigger::Event(BattleEvent::Kind(
        crate::engine::event::kind::EventKind::TargetAttacked,
    ));
    let owner = FrameOwner::BuffAct {
        owner_uid: -1,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 30,
        key: DefinitionKey::new(1028, "RealDamageKill"),
    };
    let cases = [
        ("Command", FrameOwner::Command),
        (
            "EventEffect",
            FrameOwner::EventEffect {
                source_uid: 10,
                target_uid: -2,
            },
        ),
    ];
    for (label, losing_frame_owner) in cases {
        let mut managers = BattleManagers::seeded(&fight);
        assert!(managers.commit_terminal(crate::engine::round::outcome::BattleOutcome::Victory));
        let mut frames = Vec::new();
        let path = push_root(&mut frames, owner.clone(), frame_trigger(&trigger));
        let mut queue = VecDeque::from([
            QueuedOp {
                op: RuleOp::BuffFeatureMarker {
                    target_uid: -1,
                    effect_type: sonettobuf::effect_type_enum::EffectType::Realdamagekill as i32,
                    effect_num: 9999,
                    buff_act_id: 0,
                },
                trigger: trigger.clone(),
                skill_execution: None,
                frame_path: None,
                parent_path: None,
                frame_group: Some(Rc::new(RefCell::new(Some(path.clone())))),
                independent_parent_group: None,
                frame_owner: Some(owner.clone()),
                subscriber_owner_uid: Some(-1),
            },
            QueuedOp {
                op: RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(
                    ExPointChange {
                        origin: CommandOrigin {
                            domain: RuleDomain::Behavior,
                            key: DefinitionKey::new(0, "QueuedLoserTest"),
                        },
                        source_uid: -2,
                        target_uid: -2,
                        delta: 1,
                        config_effect: 0,
                        effect_type: 0,
                    },
                ))),
                trigger: trigger.clone(),
                skill_execution: None,
                frame_path: Some(path),
                parent_path: None,
                frame_group: Some(Rc::new(RefCell::new(None))),
                independent_parent_group: None,
                frame_owner: Some(losing_frame_owner),
                subscriber_owner_uid: Some(-2),
            },
        ]);

        let result = drain_queue_with_frames(
            &mut managers,
            &pool,
            &SkillEffectCatalog::default(),
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            &mut queue,
            frames,
        )
        .unwrap();

        assert!(
            result.outcomes.iter().any(|outcome| matches!(
                outcome,
                RuleOutcome::BuffFeatureMarker(marker)
                    if marker.target_uid == -1
                        && marker.effect_type
                            == sonettobuf::effect_type_enum::EffectType::Realdamagekill as i32
            )),
            "populated group must remain allowed for {label}"
        );
        assert!(
            !result.events.iter().any(|event| matches!(
                event,
                BattleEvent::ExPointChanged(change) if change.source_uid == -2
            )),
            "unstarted losing subscriber must be skipped for {label}"
        );
    }
}

#[test]
fn lethal_damage_is_settled_after_a_configured_survival_reaction() {
    crate::test_support::init_config();
    let entity = |uid, model_id, hp| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(hp),
        attr: Some(HeroAttribute {
            hp: Some(hp),
            attack: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut defender =
        crate::engine::fight::defender::Defender::build_monster_with_uid(30111001, -1, 1, 2)
            .unwrap();
    defender.current_hp = Some(1);
    defender.attr = Some(HeroAttribute {
        hp: Some(1),
        attack: Some(100),
        ..Default::default()
    });
    defender.base_attr = defender.attr;
    defender.passive_skill.push(530000151);
    let fight = Fight {
        battle_id: Some(301110),
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 100)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![defender],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let result = run_command_group(
        &mut managers,
        &pool,
        crate::engine::skill::effect::catalog::global(),
        &mut RoundDeterminism::default(),
        TargetContext {
            battle_id: 301110,
            ..Default::default()
        },
        [RuleOp::Command(BattleCommand::Hp(
            crate::engine::manager::hp::HpCommand::Damage(crate::engine::manager::hp::HpDamage {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(0, "TestDamage"),
                },
                source_uid: 10,
                target_uid: -1,
                amount: 1,
                config_effect: 1,
                effect_kind: crate::engine::manager::hp::DamageEffectKind::Normal,
                assassinate: false,
                ignore_riposte: false,
                hurt: crate::engine::manager::hp::HurtInfoData {
                    from_uid: 10,
                    is_crit: false,
                    career_restraint: false,
                    reduce_hp: 1,
                    effect_id: 1,
                    skill_id: 1,
                    damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
                    buff_act_id: 0,
                    buff_uid: 0,
                    hurt_effect_type: 0,
                    display_amount: None,
                },
            }),
        ))],
    )
    .unwrap();

    fn contains_death(frame: &SemanticFrame) -> bool {
        frame.items.iter().any(|item| match item {
            crate::engine::runtime::record::FrameItem::Change(change) => {
                matches!(change.as_ref(), BattleChange::Death(_))
            }
            crate::engine::runtime::record::FrameItem::Child(child) => contains_death(child),
            crate::engine::runtime::record::FrameItem::Cue(_) => false,
        })
    }

    assert_eq!(managers.entity.model_id(-1), Some(30111005));
    assert!(managers.hp.current(-1) > 0);
    assert!(
        managers
            .entity
            .passive_override(-1)
            .is_some_and(|skills| skills.contains(&530000151))
    );
    assert!(result.outcomes.iter().any(|outcome| matches!(
        outcome,
        RuleOutcome::Buff(changes)
            if changes.change.rejected.as_ref().is_some_and(|rejected|
                rejected.buff.buff_id == Some(530000111)
                    && rejected.blocker_buff_id == 530000417
            )
    )));
    assert!(!result.frames.iter().any(contains_death));
    assert!(
        !result
            .events
            .iter()
            .any(|event| matches!(event, BattleEvent::BattleTerminalCommitted { .. }))
    );
}

#[test]
fn terminal_commit_keeps_winner_reactions_and_completes_the_current_action() {
    crate::test_support::init_config();
    let entity = |uid, model_id, hp, passive_skill| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(hp),
        ex_point: Some(0),
        passive_skill,
        attr: Some(HeroAttribute {
            hp: Some(hp),
            attack: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut terminal_target = entity(-1, 30111005, 1, Vec::new());
    terminal_target.buffs = vec![BuffInfo {
        uid: Some(3),
        buff_id: Some(302),
        from_uid: Some(-1),
        count: Some(1),
        ..Default::default()
    }];
    let fight = Fight {
        battle_id: Some(301110),
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 100, vec![100])],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![terminal_target, entity(-2, 2, 100, vec![101])],
            ..Default::default()
        }),
        ..Default::default()
    };
    let death_slot = |opcode, type_name: &str| {
        let mut slot = SkillEffectSlot::new(
            ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
            TargetRequest::self_only(),
        );
        slot.conditions = vec![ParsedCondition {
            opcode,
            type_name: type_name.to_owned(),
            kind: crate::engine::skill::condition::registry::parse(opcode, type_name, &[]).unwrap(),
            raw_args: Vec::new(),
        }];
        slot.compiled_route = ConditionRoute::compile(&slot.conditions);
        slot
    };
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![death_slot(86, "EnemyDead")],
    });
    catalog.insert(ParsedSkillEffect {
        skill_id: 101,
        slots: vec![death_slot(17, "TeammateDead")],
    });
    catalog.insert(ParsedSkillEffect {
        skill_id: 200,
        slots: Vec::new(),
    });
    catalog.insert_damage_rate(200, 1_000);
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut attack: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 200,
    }
    .into();
    attack.mode = SkillExecutionMode::Active;
    attack.target = SkillTarget::Explicit(-1);

    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            battle_id: 301110,
            ..Default::default()
        },
        [RuleOp::Skill(attack.clone())],
    )
    .unwrap();

    assert!(result.events.iter().any(|event| matches!(
        event,
        BattleEvent::ExPointChanged(change) if change.source_uid == 10
    )));
    let losing_reaction = result
        .events
        .iter()
        .position(|event| {
            matches!(
                event,
                BattleEvent::ExPointChanged(change) if change.source_uid == -2
            )
        })
        .unwrap();
    let terminal_commit = result
        .events
        .iter()
        .position(|event| matches!(event, BattleEvent::BattleTerminalCommitted { .. }))
        .unwrap();
    assert!(losing_reaction < terminal_commit);
    assert_eq!(
        result
            .events
            .iter()
            .filter(|event| matches!(event, BattleEvent::BattleTerminalCommitted { .. }))
            .count(),
        1
    );
    assert!(result.outcomes.iter().any(|outcome| matches!(
        outcome,
        RuleOutcome::SkillLifecycle(
            crate::engine::skill::action::SkillLifecycle::ActionCompleted(action)
        ) if action.source_uid == 10 && action.skill_id == 200
    )));
    assert!(managers.buff.snapshot(-1, 3).is_some());

    let next = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            battle_id: 301110,
            ..Default::default()
        },
        [RuleOp::Skill(attack)],
    )
    .unwrap();
    assert!(!next.events.iter().any(|event| matches!(
        event,
        BattleEvent::BattleTerminalCommitted { .. } | BattleEvent::SkillAction(_)
    )));
}
