use super::*;

fn attack_crit_route_catalog(condition_opcode: i32) -> SkillEffectCatalog {
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(50008, "DirectUseSkill", vec![100]),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: condition_opcode,
        type_name: "AttackCrit".to_owned(),
        kind: crate::engine::skill::condition::registry::parse(condition_opcode, "AttackCrit", &[])
            .unwrap(),
        raw_args: Vec::new(),
    }];
    slot.compiled_route =
        crate::engine::skill::rule::route::ConditionRoute::compile(&slot.conditions);

    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![slot],
    });
    catalog
}

fn attack_crit_reaction(condition_opcode: i32) -> SkillInvocation {
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    invocation.condition_key = Some(DefinitionKey::new(condition_opcode, "AttackCrit"));
    invocation.phase = Some(SkillPhase::AfterDamage);
    invocation
}

fn skill_action_event(extra_skill_kind: i32, crit_count: i32) -> BattleEvent {
    BattleEvent::SkillAction(crate::engine::skill::action::SkillActionEvent {
        source_uid: 10,
        skill_id: 100,
        target_uid: 10,
        target_uids: vec![10],
        attacked_target_uids: vec![10],
        phase: SkillPhase::AfterDamage,
        skill_slot: 1,
        is_attack: true,
        rank: 1,
        skill_type: 1,
        effect_tag: 1,
        assassinate: false,
        ignore_riposte: false,
        damage_amount: 1,
        kill_count: 0,
        crit_count,
        guard_break_count: 0,
        additional_moxie: 0,
        extra_skill_kind,
        mode: SkillExecutionMode::Active,
        teammate_injury_count: 0,
        teammate_injury_count_not_reset: 0,
        team_injury_count_round: 0,
        card_enchants: Vec::new(),
        buff_additions: Vec::new(),
    })
}

fn attack_crit_fight() -> Fight {
    Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn child_skills(ops: Vec<RuleOp>) -> Vec<SkillInvocation> {
    ops.into_iter()
        .filter_map(|op| match op {
            RuleOp::Skill(invocation) => Some(invocation),
            _ => None,
        })
        .collect()
}

#[test]
fn initial_attack_crit_emits_one_follow_up_child() {
    crate::test_support::init_config();
    let fight = attack_crit_fight();
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = attack_crit_route_catalog(30402);

    let children = child_skills(
        emit_all_ops(
            attack_crit_reaction(30402),
            &managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            &SkillOpTrigger::Event(skill_action_event(0, 1)),
        )
        .unwrap(),
    );

    assert_eq!(children.len(), 1);
    assert_eq!(
        children[0].extra_skill_kind,
        Some(crate::engine::skill::condition::extra::ExtraSkillKind::FollowUp)
    );
}

#[test]
fn critical_follow_up_does_not_recurse_or_exhaust_the_drain_budget() {
    crate::test_support::init_config();
    let fight = attack_crit_fight();
    let pool = TargetPool::from_fight(&fight);
    let catalog = attack_crit_route_catalog(30402);
    let mut managers = BattleManagers::seeded(&fight);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    invocation.target = SkillTarget::Explicit(10);

    let result = crate::engine::runtime::drain::run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            action_crit_count: 1,
            ..Default::default()
        },
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    let completed_actions = result
        .outcomes
        .iter()
        .filter(|outcome| {
            matches!(
                outcome,
                crate::engine::runtime::executor::RuleOutcome::SkillLifecycle(
                    crate::engine::skill::action::SkillLifecycle::ActionCompleted(_)
                )
            )
        })
        .count();
    assert_eq!(completed_actions, 2);
}

#[test]
fn nearby_attack_crit_route_still_triggers_for_an_extra_action() {
    crate::test_support::init_config();
    let fight = attack_crit_fight();
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = attack_crit_route_catalog(30208);

    let children = child_skills(
        emit_all_ops(
            attack_crit_reaction(30208),
            &managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            &SkillOpTrigger::Event(skill_action_event(2, 1)),
        )
        .unwrap(),
    );

    assert_eq!(children.len(), 1);
    assert_eq!(children[0].extra_skill_kind, None);
}

#[test]
fn ally_action_context_preserves_assassination_identity() {
    let mut context = TargetContext::default();
    let event = BattleEvent::AllyAction(crate::engine::skill::action::ActionEvent {
        source_uid: 10,
        skill_id: 100,
        assassinate: true,
        ..Default::default()
    });

    super::super::invoke::apply_event_context(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        &mut context,
        &event,
    );

    assert_eq!(context.active_skill_source_uid, 10);
    assert_eq!(context.active_skill_id, 100);
    assert!(context.active_skill_assassinate);
}

#[test]
fn hit_context_uses_the_explicit_skill_catalog_rank() {
    let db = crate::test_support::game_data();
    let skill_id = 30_230_111;
    let mut context = TargetContext::default();
    let event = BattleEvent::Hit(crate::engine::event::payload::HitEvent {
        origin: crate::engine::skill::rule::CommandOrigin {
            domain: crate::engine::skill::rule::RuleDomain::Behavior,
            key: crate::engine::skill::rule::DefinitionKey::new(10_005, "Damage"),
        },
        source_uid: 10,
        target_uid: -1,
        skill_id,
        amount: 1,
        shield_absorbed: 0,
        career_restraint: true,
        damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
        assassinate: false,
        ignore_riposte: false,
    });

    super::super::invoke::apply_event_context(
        crate::catalog::BattleCatalog::new(db),
        &mut context,
        &event,
    );

    assert_eq!(
        context.active_skill_rank,
        db.skill.get(skill_id).unwrap().skill_rank
    );
    assert_eq!(context.hit_career_restraint, Some(true));
}

#[test]
fn event_trigger_satisfies_only_its_driver_and_keeps_other_conditions() {
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
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let slot = |delta, threshold| {
        let mut slot = SkillEffectSlot::new(
            ParsedBehavior::from_spec(
                BehaviorSpec::new(60189, "AddEnergyToCard"),
                vec![1, delta, 1],
                Vec::new(),
            ),
            TargetRequest::self_only(),
        );
        slot.conditions = vec![
            ParsedCondition {
                opcode: 208,
                type_name: "None".to_owned(),
                kind: ParsedConditionKind::None(
                    crate::engine::skill::condition::none::NoneMode::SkillAction,
                ),
                raw_args: Vec::new(),
            },
            ParsedCondition {
                opcode: 1209,
                type_name: "LifeLess".to_owned(),
                kind: ParsedConditionKind::HpPermille {
                    compare: ConditionCompare::LessThan,
                    threshold,
                },
                raw_args: Vec::new(),
            },
        ];
        slot.compiled_route =
            crate::engine::skill::rule::route::ConditionRoute::compile(&slot.conditions);
        slot
    };
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![slot(1, 500), slot(2, 1500)],
    });
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    invocation.condition_key = Some(DefinitionKey::new(208, "None"));
    invocation.phase = Some(crate::engine::skill::action::SkillPhase::AfterDamage);
    let event = BattleEvent::Kind(EventKind::SkillAction);

    let mut wrong_phase = invocation.clone();
    wrong_phase.phase = Some(crate::engine::skill::action::SkillPhase::Immediate);
    assert!(
        emit_all_ops(
            wrong_phase,
            &managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            &SkillOpTrigger::Event(event.clone()),
        )
        .unwrap()
        .is_empty()
    );

    let ops = emit_all_ops(
        invocation.clone(),
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &SkillOpTrigger::Event(event),
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Card(
            CardCommand::ChangeBasicEnergy(CardEnergyChange { delta: 2, .. })
        ))]
    ));
}

#[test]
fn event_conditions_use_the_deterministic_random_stream() {
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
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(
            BehaviorSpec::new(60189, "AddEnergyToCard"),
            vec![1, 2, 1],
            Vec::new(),
        ),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![
        ParsedCondition {
            opcode: 208,
            type_name: "None".to_owned(),
            kind: ParsedConditionKind::None(
                crate::engine::skill::condition::none::NoneMode::SkillAction,
            ),
            raw_args: Vec::new(),
        },
        ParsedCondition {
            opcode: 552210,
            type_name: "Random".to_owned(),
            kind: ParsedConditionKind::Random { threshold: 500 },
            raw_args: vec!["500".to_owned()],
        },
    ];
    slot.compiled_route =
        crate::engine::skill::rule::route::ConditionRoute::compile(&slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![slot],
    });
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    invocation.condition_key = Some(DefinitionKey::new(208, "None"));
    invocation.phase = Some(crate::engine::skill::action::SkillPhase::AfterDamage);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_condition_random_choices(vec![
        crate::engine::runtime::determinism::ConditionRandomChoice {
            skill_id: 100,
            opcode: 552210,
            roll: 499,
        },
    ]);

    let ops = emit_all_ops(
        invocation.clone(),
        &managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
        &SkillOpTrigger::Event(BattleEvent::Kind(EventKind::SkillAction)),
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Card(
            CardCommand::ChangeBasicEnergy(CardEnergyChange { delta: 2, .. })
        ))]
    ));
}

#[test]
fn event_subscriber_does_not_publish_a_second_skill_action() {
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
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::from_spec(
            BehaviorSpec::new(60189, "AddEnergyToCard"),
            vec![1, 2, 1],
            Vec::new(),
        ),
        TargetRequest::self_only(),
    );
    slot.conditions = vec![ParsedCondition {
        opcode: 210,
        type_name: "None".to_owned(),
        kind: ParsedConditionKind::None(
            crate::engine::skill::condition::none::NoneMode::SkillActionAfterHit,
        ),
        raw_args: Vec::new(),
    }];
    slot.compiled_route =
        crate::engine::skill::rule::route::ConditionRoute::compile(&slot.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![slot],
    });
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    invocation.condition_key = Some(DefinitionKey::new(210, "None"));
    invocation.phase = Some(SkillPhase::AfterHit);

    let emission = emit_ops(
        invocation.clone(),
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        &mut SkillExecution::new(TargetContext::default()),
        &SkillOpTrigger::Event(BattleEvent::Kind(EventKind::SkillAction)),
    )
    .unwrap();

    assert!(emission.ops.iter().any(|emission| {
        matches!(
            emission.op,
            RuleOp::Command(BattleCommand::Card(CardCommand::ChangeBasicEnergy(_)))
        )
    }));
    assert!(
        emission
            .ops
            .iter()
            .all(|emission| !matches!(emission.op, RuleOp::SkillLifecycle(_)))
    );

    invocation.condition_key = None;
    let setup = emit_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        &mut SkillExecution::new(TargetContext::default()),
        &SkillOpTrigger::Setup {
            stage: SetupStage::EnterFight,
            priority: 0,
        },
    )
    .unwrap();
    assert!(
        setup
            .ops
            .iter()
            .all(|emission| !matches!(emission.op, RuleOp::SkillLifecycle(_)))
    );
}

#[test]
fn event_context_does_not_override_a_reactive_skills_own_target_rule() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::from_spec(
                BehaviorSpec::new(60189, "AddEnergyToCard"),
                vec![1, 2, 1],
                Vec::new(),
            ),
            TargetRequest {
                code: crate::engine::skill::target::request::SOURCE_TARGET_CODE,
                raw: Vec::new(),
            },
        )],
    });
    let event = BattleEvent::BuffAdded(crate::engine::event::payload::BuffChangeEvent {
        source_uid: 20,
        target_uid: 20,
        buff_uid: 1,
        buff_id: 1,
        before_amount: 0,
        after_amount: 1,
        act_id: 0,
        act_value: 0,
    });

    let emission = emit_ops(
        SkillRequest {
            source_uid: 10,
            skill_id: 100,
        }
        .into(),
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        &mut SkillExecution::new(TargetContext::default()),
        &SkillOpTrigger::Event(event),
    )
    .unwrap();

    assert_eq!(emission.target_uid, Some(10));
}

#[test]
fn active_child_skill_enters_the_phase_required_by_an_owner_passive() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                passive_skill: vec![1_249_101],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
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
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 12_410_011,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    invocation.phase = Some(SkillPhase::AdditionalDamage);
    invocation.target = SkillTarget::Explicit(-1);
    let mut execution = SkillExecution::new(TargetContext::default());
    execution.configured_targets = Some(vec![-1]);

    let emission = emit_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        &mut execution,
        &SkillOpTrigger::Active,
    )
    .unwrap();

    assert_eq!(
        emission.continuation.and_then(|next| next.phase),
        Some(SkillPhase::AfterDamage)
    );
}
